use clap::{Parser, ValueEnum};
use jl_sg::InquiryData;
use jl_sg::ScsiTransport;
use jl_sg::TransportError;
use jl_sg::candidate_devices;
use jl_sg::linux::{LinuxSgDevice, resolve_selector_to_sg_path};
use jl_uboot::{Error as UbootError, JlDevice, MemAccess, ProtocolKind};
use std::io::Write;

type BoxError = Box<dyn std::error::Error>;

#[derive(Parser, Debug)]
#[command(name = "jlrunner")]
#[command(about = "Experimental JieLi RAM loader runner")]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = CliProtocol::Loaderv2)]
    protocol: CliProtocol,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u32,
    #[arg(long)]
    device: Option<String>,
    #[arg(long)]
    address: u32,
    #[arg(long)]
    file: String,
    #[arg(long, default_value_t = 4096)]
    chunk_size: usize,
    #[arg(long, default_value_t = false)]
    progress: bool,
    #[arg(long, default_value_t = 0)]
    arg: u32,
    #[arg(long, default_value_t = true)]
    jump: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliProtocol {
    Loaderv2,
    Loaderv1,
    Uboot1,
}

#[derive(Clone, Copy, Debug)]
enum ExitCode {
    Io = 10,
    DeviceNotFound = 11,
    Transport = 12,
    Protocol = 13,
}

#[derive(Clone, Debug)]
struct ResolvedDevice {
    selector: String,
    transport_path: String,
}

pub trait RunnerDeviceOps {
    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError>;
    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), UbootError>;
}

impl<T: ScsiTransport + 'static> RunnerDeviceOps for JlDevice<T> {
    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError> {
        MemAccess::mem_write(self, addr, data)
    }

    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), UbootError> {
        MemAccess::mem_jump(self, addr, arg)
    }
}

pub trait DeviceFactory {
    fn open(
        &self,
        device: &str,
        timeout_ms: u32,
        protocol: ProtocolKind,
    ) -> Result<Box<dyn RunnerDeviceOps>, BoxError>;
}

pub struct LinuxDeviceFactory;

impl DeviceFactory for LinuxDeviceFactory {
    fn open(
        &self,
        device: &str,
        timeout_ms: u32,
        protocol: ProtocolKind,
    ) -> Result<Box<dyn RunnerDeviceOps>, BoxError> {
        let dev = LinuxSgDevice::open_with_timeout(device, timeout_ms)?;
        Ok(Box::new(JlDevice::new(dev, protocol)))
    }
}

pub fn run_cli(cli: Cli) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    run_cli_with_factory(&cli, &LinuxDeviceFactory, &mut stdout, &mut stderr)
}

pub fn run_cli_with_factory(
    cli: &Cli,
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match run_with_factory(cli, factory, out, err) {
        Ok(()) => 0,
        Err(err0) => {
            let err_ref = err0.as_ref();
            let _ = writeln!(err, "error: {err_ref}");
            exit_code_for(err_ref) as i32
        }
    }
}

fn run_with_factory(
    cli: &Cli,
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), BoxError> {
    let image = std::fs::read(&cli.file)?;
    let resolved = resolve_device_selector(cli.device.as_deref(), cli.timeout_ms)?;
    let mut jl = factory.open(
        &resolved.transport_path,
        cli.timeout_ms,
        cli.protocol.into(),
    )?;

    write_memory_chunked(
        jl.as_mut(),
        err,
        cli.address,
        &image,
        cli.chunk_size,
        cli.progress,
    )?;
    writeln!(
        out,
        "loaded {} bytes from {} into RAM at 0x{:08x} via {}",
        image.len(),
        cli.file,
        cli.address,
        resolved.selector
    )?;

    if cli.jump {
        jl.mem_jump(cli.address, cli.arg)?;
        writeln!(out, "jumped to 0x{:08x} arg=0x{:08x}", cli.address, cli.arg)?;
    }

    Ok(())
}

fn exit_code_for(err: &(dyn std::error::Error + 'static)) -> ExitCode {
    if err.downcast_ref::<std::io::Error>().is_some() {
        return ExitCode::Io;
    }
    if let Some(te) = err.downcast_ref::<TransportError>() {
        return match te {
            TransportError::DeviceNotFound(_) => ExitCode::DeviceNotFound,
            TransportError::Io(_) => ExitCode::Io,
            _ => ExitCode::Transport,
        };
    }
    if let Some(ue) = err.downcast_ref::<UbootError>() {
        return match ue {
            UbootError::Transport(te) => match te {
                TransportError::DeviceNotFound(_) => ExitCode::DeviceNotFound,
                TransportError::Io(_) => ExitCode::Io,
                _ => ExitCode::Transport,
            },
            _ => ExitCode::Protocol,
        };
    }
    ExitCode::Protocol
}

fn progress_step(err: &mut dyn Write, enabled: bool, label: &str, done: usize, total: usize) {
    if enabled {
        let _ = writeln!(err, "progress {label}: {done}/{total} bytes");
    }
}

fn resolve_device_selector(
    selector: Option<&str>,
    timeout_ms: u32,
) -> Result<ResolvedDevice, BoxError> {
    if let Some(selector) = selector {
        return resolve_explicit_selector(selector);
    }
    auto_detect_device(timeout_ms)
}

fn resolve_explicit_selector(selector: &str) -> Result<ResolvedDevice, BoxError> {
    #[cfg(target_os = "linux")]
    {
        let transport_path = resolve_selector_to_sg_path(selector)?;
        Ok(ResolvedDevice {
            selector: selector.to_string(),
            transport_path: transport_path.display().to_string(),
        })
    }

    #[cfg(target_os = "windows")]
    {
        if looks_like_windows_volume_selector(selector) {
            let transport_path = jl_sg::windows::resolve_visible_selector_to_path(selector)?;
            return Ok(ResolvedDevice {
                selector: selector.to_string(),
                transport_path: transport_path.display().to_string(),
            });
        }
        return Ok(ResolvedDevice {
            selector: selector.to_string(),
            transport_path: selector.to_string(),
        });
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Ok(ResolvedDevice {
            selector: selector.to_string(),
            transport_path: selector.to_string(),
        })
    }
}

fn auto_detect_device(timeout_ms: u32) -> Result<ResolvedDevice, BoxError> {
    select_auto_detect_candidate(candidate_devices()?, |candidate| {
        let mut dev = match open_platform_scsi(&candidate.path, timeout_ms) {
            Ok(dev) => dev,
            Err(_) => return false,
        };
        let inquiry = match dev.inquiry() {
            Ok(inquiry) => inquiry,
            Err(_) => return false,
        };
        matches_jieli_candidate(&inquiry)
    })
}

fn select_auto_detect_candidate<F>(
    candidates: Vec<jl_sg::ScsiCandidate>,
    mut is_match: F,
) -> Result<ResolvedDevice, BoxError>
where
    F: FnMut(&jl_sg::ScsiCandidate) -> bool,
{
    let matches = candidates
        .into_iter()
        .filter(|candidate| is_match(candidate))
        .collect();
    resolve_auto_detect_matches(matches)
}

fn resolve_auto_detect_matches(
    mut matches: Vec<jl_sg::ScsiCandidate>,
) -> Result<ResolvedDevice, BoxError> {
    match matches.len() {
        0 => Err(Box::new(TransportError::DeviceNotFound(
            "no matching JieLi download-mode target found".to_string(),
        ))),
        1 => {
            let candidate = matches.pop().unwrap();
            Ok(ResolvedDevice {
                selector: selector_for_candidate(&candidate),
                transport_path: candidate.path.display().to_string(),
            })
        }
        n => Err(Box::new(TransportError::DeviceNotFound(format!(
            "multiple matching JieLi targets found ({n}); pass --device"
        )))),
    }
}

fn selector_for_candidate(candidate: &jl_sg::ScsiCandidate) -> String {
    candidate
        .selector
        .clone()
        .or_else(|| candidate.note.clone())
        .unwrap_or_else(|| candidate.path.display().to_string())
}

fn matches_jieli_candidate(inquiry: &InquiryData) -> bool {
    inquiry.product.starts_with("UBOOT")
        || inquiry.product.starts_with("UDISK")
        || inquiry.product.starts_with("DEVICE")
}

#[cfg(target_os = "linux")]
fn open_platform_scsi(
    path: &std::path::Path,
    timeout_ms: u32,
) -> Result<LinuxSgDevice, TransportError> {
    LinuxSgDevice::open_with_timeout(path, timeout_ms)
}

#[cfg(target_os = "windows")]
fn open_platform_scsi(
    path: &std::path::Path,
    timeout_ms: u32,
) -> Result<jl_sg::windows::WindowsScsiDevice, TransportError> {
    jl_sg::windows::WindowsScsiDevice::open_with_timeout(path, timeout_ms)
}

#[cfg(target_os = "windows")]
fn looks_like_windows_volume_selector(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

impl From<CliProtocol> for ProtocolKind {
    fn from(value: CliProtocol) -> Self {
        match value {
            CliProtocol::Loaderv2 => ProtocolKind::LoaderV2,
            CliProtocol::Loaderv1 => ProtocolKind::LoaderV1,
            CliProtocol::Uboot1 => ProtocolKind::Uboot1,
        }
    }
}

fn write_memory_chunked(
    jl: &mut dyn RunnerDeviceOps,
    err: &mut dyn Write,
    address: u32,
    image: &[u8],
    chunk_size: usize,
    progress: bool,
) -> Result<(), BoxError> {
    let chunk_size = chunk_size.max(1);
    for (idx, chunk) in image.chunks(chunk_size).enumerate() {
        let addr = address.wrapping_add((idx * chunk_size) as u32);
        jl.mem_write(addr, chunk)?;
        let done = ((idx + 1) * chunk_size).min(image.len());
        progress_step(err, progress, "mem-write", done, image.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    type WriteLog = Arc<Mutex<Vec<(u32, Vec<u8>)>>>;
    type JumpLog = Arc<Mutex<Vec<(u32, u32)>>>;

    #[derive(Clone, Default)]
    struct FakeDevice {
        writes: WriteLog,
        jumps: JumpLog,
    }

    impl RunnerDeviceOps for FakeDevice {
        fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError> {
            self.writes.lock().unwrap().push((addr, data.to_vec()));
            Ok(())
        }

        fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), UbootError> {
            self.jumps.lock().unwrap().push((addr, arg));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeFactory {
        device: Option<FakeDevice>,
        err: Option<TransportError>,
    }

    impl DeviceFactory for FakeFactory {
        fn open(
            &self,
            _device: &str,
            _timeout_ms: u32,
            _protocol: ProtocolKind,
        ) -> Result<Box<dyn RunnerDeviceOps>, BoxError> {
            if let Some(err) = &self.err {
                return Err(Box::new(match err {
                    TransportError::Unsupported => TransportError::Unsupported,
                    TransportError::DeviceNotFound(s) => TransportError::DeviceNotFound(s.clone()),
                    TransportError::Io(e) => {
                        TransportError::Io(std::io::Error::new(e.kind(), e.to_string()))
                    }
                    TransportError::InvalidCdbLength(v) => TransportError::InvalidCdbLength(*v),
                    TransportError::SgIo(s) => TransportError::SgIo(s.clone()),
                    TransportError::WinScsi(s) => TransportError::WinScsi(s.clone()),
                    TransportError::InvalidInquiry => TransportError::InvalidInquiry,
                }));
            }
            Ok(Box::new(self.device.clone().unwrap()))
        }
    }

    fn fake_linux_selector() -> String {
        let tmp = tempdir().unwrap();
        let path = tmp.keep().join("sg3");
        std::fs::write(&path, []).unwrap();
        path.display().to_string()
    }

    #[test]
    fn success_path_writes_and_jumps_with_progress() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), [0x11, 0x22, 0x33, 0x44]).unwrap();
        let selector = fake_linux_selector();

        let cli = Cli::parse_from([
            "jlrunner",
            "--device",
            &selector,
            "--address",
            "4096",
            "--file",
            tmp.path().to_str().unwrap(),
            "--progress",
        ]);
        let device = FakeDevice::default();
        let writes_ref = device.writes.clone();
        let jumps_ref = device.jumps.clone();
        let factory = FakeFactory {
            device: Some(device),
            err: None,
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();

        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 0);
        assert_eq!(writes_ref.lock().unwrap().len(), 1);
        assert_eq!(writes_ref.lock().unwrap()[0].0, 4096);
        assert_eq!(
            writes_ref.lock().unwrap()[0].1,
            vec![0x11, 0x22, 0x33, 0x44]
        );
        assert_eq!(jumps_ref.lock().unwrap().as_slice(), &[(4096, 0)]);

        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("loaded 4 bytes"));
        assert!(stdout.contains("jumped to 0x00001000"));
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("progress mem-write"));
    }

    #[test]
    fn transport_failure_maps_to_exit_code_12() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), [0x11]).unwrap();
        let selector = fake_linux_selector();

        let cli = Cli::parse_from([
            "jlrunner",
            "--device",
            &selector,
            "--address",
            "0",
            "--file",
            tmp.path().to_str().unwrap(),
        ]);
        let factory = FakeFactory {
            device: None,
            err: Some(TransportError::SgIo("boom".into())),
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();

        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 12);
        assert!(String::from_utf8(stderr).unwrap().contains("boom"));
    }

    #[test]
    fn selector_for_candidate_prefers_selector_then_note_then_path() {
        let candidate = jl_sg::ScsiCandidate {
            selector: Some("E:".into()),
            path: "/dev/sg3".into(),
            vid: None,
            pid: None,
            note: Some("USBSTOR".into()),
        };
        assert_eq!(selector_for_candidate(&candidate), "E:");

        let candidate = jl_sg::ScsiCandidate {
            selector: None,
            path: "/dev/sg3".into(),
            vid: None,
            pid: None,
            note: Some("USBSTOR".into()),
        };
        assert_eq!(selector_for_candidate(&candidate), "USBSTOR");

        let candidate = jl_sg::ScsiCandidate {
            selector: None,
            path: "/dev/sg3".into(),
            vid: None,
            pid: None,
            note: None,
        };
        assert_eq!(selector_for_candidate(&candidate), "/dev/sg3");
    }

    #[test]
    fn auto_detect_match_resolution_handles_zero_single_and_multiple() {
        let no_match = resolve_auto_detect_matches(Vec::new()).unwrap_err();
        assert!(no_match.to_string().contains("no matching JieLi"));

        let single = resolve_auto_detect_matches(vec![jl_sg::ScsiCandidate {
            selector: Some("E:".into()),
            path: "/dev/sg3".into(),
            vid: Some(0x4A4C),
            pid: Some(0x00F1),
            note: None,
        }])
        .unwrap();
        assert_eq!(single.selector, "E:");
        assert_eq!(single.transport_path, "/dev/sg3");

        let multiple = resolve_auto_detect_matches(vec![
            jl_sg::ScsiCandidate {
                selector: Some("E:".into()),
                path: "/dev/sg3".into(),
                vid: None,
                pid: None,
                note: None,
            },
            jl_sg::ScsiCandidate {
                selector: Some("F:".into()),
                path: "/dev/sg4".into(),
                vid: None,
                pid: None,
                note: None,
            },
        ])
        .unwrap_err();
        assert!(
            multiple
                .to_string()
                .contains("multiple matching JieLi targets found (2)")
        );
    }

    #[test]
    fn auto_detect_filter_only_keeps_matching_candidates() {
        let resolved = select_auto_detect_candidate(
            vec![
                jl_sg::ScsiCandidate {
                    selector: Some("not-jieli".into()),
                    path: "/dev/sg2".into(),
                    vid: None,
                    pid: None,
                    note: None,
                },
                jl_sg::ScsiCandidate {
                    selector: Some("JIELI-DL".into()),
                    path: "/dev/sg3".into(),
                    vid: None,
                    pid: None,
                    note: None,
                },
            ],
            |candidate| candidate.selector.as_deref() == Some("JIELI-DL"),
        )
        .unwrap();

        assert_eq!(resolved.selector, "JIELI-DL");
        assert_eq!(resolved.transport_path, "/dev/sg3");
    }
}
