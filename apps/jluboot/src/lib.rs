use clap::{Parser, Subcommand, ValueEnum};
use jl_sg::linux::{LinuxSgDevice, resolve_selector_to_sg_path};
use jl_sg::{InquiryData, ScsiCandidate, ScsiTransport, TransportError, candidate_devices};
use jl_uboot::{
    ChipKeyRaw, Error as UbootError, FlashAccess, JlDevice, LoaderV1FlashSelect, MemAccess,
    OnlineDeviceInfo, ProtocolKind,
};
use serde_json::json;
use std::io::Write;
use thiserror::Error;

type BoxError = Box<dyn std::error::Error>;

#[derive(Parser, Debug)]
#[command(name = "jluboot")]
#[command(about = "Experimental JieLi Linux flasher CLI scaffold")]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = CliProtocol::Loaderv2)]
    protocol: CliProtocol,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u32,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    yes: bool,
    #[arg(long, default_value_t = false)]
    progress: bool,
    #[arg(long, default_value_t = 4096)]
    chunk_size: usize,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliProtocol {
    Loaderv2,
    Loaderv1,
    Uboot1,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FlashSelectKind {
    Code,
    Data,
}

#[derive(Clone, Copy, Debug)]
struct RunCtx {
    protocol: ProtocolKind,
    timeout_ms: u32,
    json_mode: bool,
    yes: bool,
    progress: bool,
    chunk_size: usize,
}

#[derive(Clone, Copy, Debug)]
struct ReadOutputOpts<'a> {
    output: Option<&'a str>,
    stdout: bool,
    hexdump: bool,
}

struct ReadRequest<'a> {
    device: Option<&'a str>,
    address: u32,
    length: usize,
    output: ReadOutputOpts<'a>,
}

#[derive(Clone, Debug)]
struct ResolvedDevice {
    selector: String,
    transport_path: String,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0} is destructive; pass --yes to continue")]
    ConfirmationRequired(String),
}

#[derive(Clone, Copy, Debug)]
enum ExitCode {
    Io = 10,
    DeviceNotFound = 11,
    Transport = 12,
    Protocol = 13,
    ConfirmationRequired = 14,
    Other = 1,
}

#[derive(Subcommand, Debug)]
enum Command {
    Find {
        #[arg(long, default_value_t = false)]
        probe: bool,
        #[arg(long)]
        vendor: Option<String>,
    },
    Probe {
        #[arg(long)]
        device: Option<String>,
    },
    FlashRead {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        length: usize,
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = false)]
        stdout: bool,
        #[arg(long, default_value_t = false)]
        hexdump: bool,
    },
    FlashWrite {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        input: String,
    },
    FlashEraseSector {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
    },
    FlashEraseBlock {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
    },
    FlashEraseChip {
        #[arg(long)]
        device: Option<String>,
    },
    ReadId {
        #[arg(long)]
        device: Option<String>,
    },
    OnlineDevice {
        #[arg(long)]
        device: Option<String>,
    },
    UsbBufferSize {
        #[arg(long)]
        device: Option<String>,
    },
    Version {
        #[arg(long)]
        device: Option<String>,
    },
    MaskromId {
        #[arg(long)]
        device: Option<String>,
    },
    ReadStatus {
        #[arg(long)]
        device: Option<String>,
    },
    FlashCrc16 {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        length: usize,
    },
    FlashCrc16Raw {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        length: usize,
    },
    SetFlashCmds {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, num_args = 8)]
        cmds: Vec<u8>,
    },
    ChipKey {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, default_value_t = 0x00ac6900)]
        arg: u32,
        #[arg(long, default_value_t = false)]
        raw: bool,
    },
    WriteChipKey {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        key: u32,
        #[arg(long, default_value_t = 5000)]
        vpp: u32,
    },
    FlashSelect {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, value_enum)]
        kind: FlashSelectKind,
    },
    MemRead {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        length: usize,
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = false)]
        stdout: bool,
        #[arg(long, default_value_t = false)]
        hexdump: bool,
    },
    MemWrite {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        input: String,
    },
    MemWriteRxgp {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long)]
        input: String,
    },
    Jump {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        address: u32,
        #[arg(long, default_value_t = 0)]
        arg: u32,
    },
    RunApp {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, default_value_t = 1)]
        arg: u32,
    },
}

pub trait DeviceOps {
    fn inquiry(&mut self) -> Result<InquiryData, UbootError>;
    fn online_device(&mut self) -> Result<OnlineDeviceInfo, UbootError>;
    fn read_id(&mut self) -> Result<u32, UbootError>;
    fn usb_buffer_size(&mut self) -> Result<u32, UbootError>;
    fn loader_version(&mut self) -> Result<Option<String>, UbootError>;
    fn maskrom_id(&mut self) -> Result<u32, UbootError>;
    fn read_status(&mut self) -> Result<u8, UbootError>;
    fn flash_crc16(&mut self, addr: u32, len: usize) -> Result<u16, UbootError>;
    fn flash_crc16_raw(&mut self, addr: u32, len: usize) -> Result<u16, UbootError>;
    fn set_flash_cmds(&mut self, cmds: &[u8]) -> Result<(), UbootError>;
    fn read_key_raw(&mut self, arg: u32) -> Result<ChipKeyRaw, UbootError>;
    fn chip_key(&mut self, arg: u32) -> Result<u16, UbootError>;
    fn write_chipkey(&mut self, key: u32, vpp: u32) -> Result<u32, UbootError>;
    fn flash_select(&mut self, sel: LoaderV1FlashSelect) -> Result<(), UbootError>;
    fn flash_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, UbootError>;
    fn flash_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError>;
    fn flash_erase_sector(&mut self, addr: u32) -> Result<(), UbootError>;
    fn flash_erase_block(&mut self, addr: u32) -> Result<(), UbootError>;
    fn flash_erase_chip(&mut self) -> Result<(), UbootError>;
    fn mem_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, UbootError>;
    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError>;
    fn mem_write_rxgp(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError>;
    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), UbootError>;
    fn run_app(&mut self, arg: u32) -> Result<(), UbootError>;
}

impl<T: ScsiTransport + 'static> DeviceOps for JlDevice<T> {
    fn inquiry(&mut self) -> Result<InquiryData, UbootError> {
        self.inquiry()
    }
    fn online_device(&mut self) -> Result<OnlineDeviceInfo, UbootError> {
        self.online_device()
    }
    fn read_id(&mut self) -> Result<u32, UbootError> {
        self.read_id()
    }
    fn usb_buffer_size(&mut self) -> Result<u32, UbootError> {
        self.usb_buffer_size()
    }
    fn loader_version(&mut self) -> Result<Option<String>, UbootError> {
        self.loader_version()
    }
    fn maskrom_id(&mut self) -> Result<u32, UbootError> {
        self.maskrom_id()
    }
    fn read_status(&mut self) -> Result<u8, UbootError> {
        self.read_status()
    }
    fn flash_crc16(&mut self, addr: u32, len: usize) -> Result<u16, UbootError> {
        self.flash_crc16(addr, len)
    }
    fn flash_crc16_raw(&mut self, addr: u32, len: usize) -> Result<u16, UbootError> {
        self.flash_crc16_raw(addr, len)
    }
    fn set_flash_cmds(&mut self, cmds: &[u8]) -> Result<(), UbootError> {
        self.set_flash_cmds(cmds)
    }
    fn read_key_raw(&mut self, arg: u32) -> Result<ChipKeyRaw, UbootError> {
        self.read_key_raw(arg)
    }
    fn chip_key(&mut self, arg: u32) -> Result<u16, UbootError> {
        self.chip_key(arg)
    }
    fn write_chipkey(&mut self, key: u32, vpp: u32) -> Result<u32, UbootError> {
        self.write_chipkey(key, vpp)
    }
    fn flash_select(&mut self, sel: LoaderV1FlashSelect) -> Result<(), UbootError> {
        self.flash_select(sel)
    }
    fn flash_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, UbootError> {
        FlashAccess::flash_read(self, addr, len)
    }
    fn flash_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError> {
        FlashAccess::flash_write(self, addr, data)
    }
    fn flash_erase_sector(&mut self, addr: u32) -> Result<(), UbootError> {
        FlashAccess::flash_erase_sector(self, addr)
    }
    fn flash_erase_block(&mut self, addr: u32) -> Result<(), UbootError> {
        FlashAccess::flash_erase_block(self, addr)
    }
    fn flash_erase_chip(&mut self) -> Result<(), UbootError> {
        FlashAccess::flash_erase_chip(self)
    }
    fn mem_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, UbootError> {
        MemAccess::mem_read(self, addr, len)
    }
    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError> {
        MemAccess::mem_write(self, addr, data)
    }
    fn mem_write_rxgp(&mut self, addr: u32, data: &[u8]) -> Result<(), UbootError> {
        self.mem_write_rxgp(addr, data)
    }
    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), UbootError> {
        MemAccess::mem_jump(self, addr, arg)
    }
    fn run_app(&mut self, arg: u32) -> Result<(), UbootError> {
        self.run_app(arg)
    }
}

pub trait DeviceFactory {
    fn open(
        &self,
        device: &str,
        timeout_ms: u32,
        protocol: ProtocolKind,
    ) -> Result<Box<dyn DeviceOps>, BoxError>;
}

pub struct LinuxDeviceFactory;

impl DeviceFactory for LinuxDeviceFactory {
    fn open(
        &self,
        device: &str,
        timeout_ms: u32,
        protocol: ProtocolKind,
    ) -> Result<Box<dyn DeviceOps>, BoxError> {
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
    let ctx = RunCtx {
        protocol: cli.protocol.into(),
        timeout_ms: cli.timeout_ms,
        json_mode: cli.json,
        yes: cli.yes,
        progress: cli.progress,
        chunk_size: cli.chunk_size.max(1),
    };

    let rc = match &cli.cmd {
        Command::Find { probe, vendor } => cmd_find(out, *probe, vendor.as_deref(), ctx),
        Command::Probe { device } => cmd_probe(factory, out, device.as_deref(), ctx),
        Command::FlashRead {
            device,
            address,
            length,
            output,
            stdout,
            hexdump,
        } => cmd_flash_read(
            factory,
            out,
            err,
            ctx,
            ReadRequest {
                device: device.as_deref(),
                address: *address,
                length: *length,
                output: ReadOutputOpts {
                    output: output.as_deref(),
                    stdout: *stdout,
                    hexdump: *hexdump,
                },
            },
        ),
        Command::FlashWrite {
            device,
            address,
            input,
        } => cmd_flash_write(factory, out, err, device.as_deref(), ctx, *address, input),
        Command::FlashEraseSector { device, address } => {
            cmd_flash_erase_sector(factory, out, device.as_deref(), ctx, *address)
        }
        Command::FlashEraseBlock { device, address } => {
            cmd_flash_erase_block(factory, out, device.as_deref(), ctx, *address)
        }
        Command::FlashEraseChip { device } => {
            cmd_flash_erase_chip(factory, out, device.as_deref(), ctx)
        }
        Command::ReadId { device } => cmd_read_id(factory, out, device.as_deref(), ctx),
        Command::OnlineDevice { device } => cmd_online_device(factory, out, device.as_deref(), ctx),
        Command::UsbBufferSize { device } => {
            cmd_usb_buffer_size(factory, out, device.as_deref(), ctx)
        }
        Command::Version { device } => cmd_version(factory, out, device.as_deref(), ctx),
        Command::MaskromId { device } => cmd_maskrom_id(factory, out, device.as_deref(), ctx),
        Command::ReadStatus { device } => cmd_read_status(factory, out, device.as_deref(), ctx),
        Command::FlashCrc16 {
            device,
            address,
            length,
        } => cmd_flash_crc16(factory, out, device.as_deref(), ctx, *address, *length),
        Command::FlashCrc16Raw {
            device,
            address,
            length,
        } => cmd_flash_crc16_raw(factory, out, device.as_deref(), ctx, *address, *length),
        Command::SetFlashCmds { device, cmds } => {
            cmd_set_flash_cmds(factory, out, device.as_deref(), ctx, cmds)
        }
        Command::ChipKey { device, arg, raw } => {
            cmd_chip_key(factory, out, device.as_deref(), ctx, *arg, *raw)
        }
        Command::WriteChipKey { device, key, vpp } => {
            cmd_write_chip_key(factory, out, device.as_deref(), ctx, *key, *vpp)
        }
        Command::FlashSelect { device, kind } => {
            cmd_flash_select(factory, out, device.as_deref(), ctx, *kind)
        }
        Command::MemRead {
            device,
            address,
            length,
            output,
            stdout,
            hexdump,
        } => cmd_mem_read(
            factory,
            out,
            err,
            ctx,
            ReadRequest {
                device: device.as_deref(),
                address: *address,
                length: *length,
                output: ReadOutputOpts {
                    output: output.as_deref(),
                    stdout: *stdout,
                    hexdump: *hexdump,
                },
            },
        ),
        Command::MemWrite {
            device,
            address,
            input,
        } => cmd_mem_write(factory, out, err, device.as_deref(), ctx, *address, input),
        Command::MemWriteRxgp {
            device,
            address,
            input,
        } => cmd_mem_write_rxgp(factory, out, err, device.as_deref(), ctx, *address, input),
        Command::Jump {
            device,
            address,
            arg,
        } => cmd_jump(factory, out, device.as_deref(), ctx, *address, *arg),
        Command::RunApp { device, arg } => cmd_run_app(factory, out, device.as_deref(), ctx, *arg),
    };

    match rc {
        Ok(()) => 0,
        Err(err0) => {
            let err_ref = err0.as_ref();
            let _ = writeln!(err, "error: {err_ref}");
            exit_code_for(err_ref) as i32
        }
    }
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

impl From<FlashSelectKind> for LoaderV1FlashSelect {
    fn from(value: FlashSelectKind) -> Self {
        match value {
            FlashSelectKind::Code => LoaderV1FlashSelect::Code,
            FlashSelectKind::Data => LoaderV1FlashSelect::Data,
        }
    }
}

fn cmd_find(
    out: &mut dyn Write,
    probe: bool,
    vendor_filter: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    cmd_find_with_candidates(out, probe, vendor_filter, ctx, &candidate_devices)
}

fn cmd_find_with_candidates(
    out: &mut dyn Write,
    probe: bool,
    vendor_filter: Option<&str>,
    ctx: RunCtx,
    candidate_source: &dyn Fn() -> Result<Vec<ScsiCandidate>, TransportError>,
) -> Result<(), BoxError> {
    let candidates = candidate_source()?;
    let mut json_entries = Vec::new();
    let mut found = 0usize;
    for candidate in candidates {
        let path = candidate.path;
        let mut dev = match open_platform_scsi(&path, ctx.timeout_ms) {
            Ok(dev) => dev,
            Err(_) => continue,
        };
        let inquiry = match dev.inquiry() {
            Ok(inquiry) => inquiry,
            Err(_) => continue,
        };
        if let Some(filter) = vendor_filter
            && !inquiry.vendor.eq_ignore_ascii_case(filter)
        {
            continue;
        }
        if !matches_jieli_candidate(&inquiry) {
            continue;
        }

        found += 1;
        let mut online_kind: Option<String> = None;
        let mut flash_id: Option<u32> = None;
        let mut loader_version: Option<String> = None;
        if probe {
            let mut jl = JlDevice::new(dev, ctx.protocol);
            let mut details = Vec::new();
            if let Ok(info) = jl.online_device() {
                online_kind = Some(format!("{:?}", info.kind));
                details.push(format!("online={:?}", info.kind));
            }
            if let Ok(id) = jl.read_id() {
                flash_id = Some(id);
                details.push(format!("flash=0x{id:06x}"));
            }
            if let Ok(version) = jl.loader_version()
                && let Some(version) = version
            {
                loader_version = Some(version.clone());
                details.push(format!("ver={version}"));
            }
            if ctx.json_mode {
                json_entries.push(build_find_json_entry(
                    &ScsiCandidate {
                        selector: candidate.selector.clone(),
                        path: path.clone(),
                        vid: candidate.vid,
                        pid: candidate.pid,
                        note: candidate.note.clone(),
                    },
                    &inquiry,
                    Some(online_kind),
                    Some(flash_id),
                    Some(loader_version),
                ));
            } else {
                write!(
                    out,
                    "{}  {} {} ({})",
                    path.display(),
                    inquiry.vendor,
                    inquiry.product,
                    inquiry.revision
                )?;
                if !details.is_empty() {
                    write!(out, "  [{}]", details.join(", "))?;
                }
                if let Some(selector) = candidate.selector.as_deref() {
                    write!(out, "  selector={selector}")?;
                }
                if let Some(vid) = candidate.vid {
                    write!(out, "  vid=0x{vid:04x}")?;
                }
                if let Some(pid) = candidate.pid {
                    write!(out, "  pid=0x{pid:04x}")?;
                }
                if let Some(note) = candidate.note.as_deref() {
                    write!(out, "  note={note}")?;
                }
                writeln!(out)?;
            }
        } else if ctx.json_mode {
            json_entries.push(build_find_json_entry(
                &ScsiCandidate {
                    selector: candidate.selector.clone(),
                    path: path.clone(),
                    vid: candidate.vid,
                    pid: candidate.pid,
                    note: candidate.note.clone(),
                },
                &inquiry,
                None,
                None,
                None,
            ));
        } else {
            writeln!(
                out,
                "{}  {} {} ({}){}{}{}{}",
                path.display(),
                inquiry.vendor,
                inquiry.product,
                inquiry.revision,
                candidate
                    .selector
                    .as_deref()
                    .map(|selector| format!("  selector={selector}"))
                    .unwrap_or_default(),
                candidate
                    .vid
                    .map(|vid| format!("  vid=0x{vid:04x}"))
                    .unwrap_or_default(),
                candidate
                    .pid
                    .map(|pid| format!("  pid=0x{pid:04x}"))
                    .unwrap_or_default(),
                candidate
                    .note
                    .as_deref()
                    .map(|note| format!("  note={note}"))
                    .unwrap_or_default(),
            )?;
        }
    }

    if ctx.json_mode {
        writeln!(out, "{}", serde_json::to_string_pretty(&json_entries)?)?;
    } else if found == 0 {
        writeln!(out, "no JieLi MSC candidates found")?;
    }
    Ok(())
}

fn build_find_json_entry(
    candidate: &ScsiCandidate,
    inquiry: &InquiryData,
    online_kind: Option<Option<String>>,
    flash_id: Option<Option<u32>>,
    loader_version: Option<Option<String>>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "path".to_string(),
        json!(candidate.path.display().to_string()),
    );
    obj.insert("selector".to_string(), json!(candidate.selector));
    obj.insert("usb_vid".to_string(), json!(candidate.vid));
    obj.insert("usb_pid".to_string(), json!(candidate.pid));
    obj.insert("note".to_string(), json!(candidate.note));
    obj.insert("vendor".to_string(), json!(inquiry.vendor));
    obj.insert("product".to_string(), json!(inquiry.product));
    obj.insert("revision".to_string(), json!(inquiry.revision));
    if let Some(online_kind) = online_kind {
        obj.insert("online_device".to_string(), json!(online_kind));
    }
    if let Some(flash_id) = flash_id {
        obj.insert("flash_id".to_string(), json!(flash_id));
    }
    if let Some(loader_version) = loader_version {
        obj.insert("loader_version".to_string(), json!(loader_version));
    }
    serde_json::Value::Object(obj)
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

fn matches_jieli_candidate(inquiry: &InquiryData) -> bool {
    inquiry.product.starts_with("UBOOT")
        || inquiry.product.starts_with("UDISK")
        || inquiry.product.starts_with("DEVICE")
}

fn open_device(
    factory: &dyn DeviceFactory,
    device: Option<&str>,
    timeout_ms: u32,
    protocol: ProtocolKind,
) -> Result<(ResolvedDevice, Box<dyn DeviceOps>), BoxError> {
    let resolved = resolve_device_selector(device, timeout_ms)?;
    let dev = factory.open(&resolved.transport_path, timeout_ms, protocol)?;
    Ok((resolved, dev))
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
    candidates: Vec<ScsiCandidate>,
    mut is_match: F,
) -> Result<ResolvedDevice, BoxError>
where
    F: FnMut(&ScsiCandidate) -> bool,
{
    let matches = candidates
        .into_iter()
        .filter(|candidate| is_match(candidate))
        .collect();
    resolve_auto_detect_matches(matches)
}

fn resolve_auto_detect_matches(
    mut matches: Vec<ScsiCandidate>,
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

fn selector_for_candidate(candidate: &ScsiCandidate) -> String {
    candidate
        .selector
        .clone()
        .or_else(|| candidate.note.clone())
        .unwrap_or_else(|| candidate.path.display().to_string())
}

#[cfg(target_os = "windows")]
fn looks_like_windows_volume_selector(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn require_yes(yes: bool, op: &str) -> Result<(), BoxError> {
    if yes {
        return Ok(());
    }
    Err(CliError::ConfirmationRequired(op.to_string()).into())
}

fn exit_code_for(err: &(dyn std::error::Error + 'static)) -> ExitCode {
    if err.is::<CliError>() {
        return ExitCode::ConfirmationRequired;
    }
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
    ExitCode::Other
}

fn progress_step(err: &mut dyn Write, enabled: bool, label: &str, done: usize, total: usize) {
    if enabled {
        let _ = writeln!(err, "progress {label}: {done}/{total} bytes");
    }
}

fn cmd_probe(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (resolved, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let inquiry = jl.inquiry()?;
    if ctx.json_mode {
        let online = jl.online_device().ok();
        let flash_id = jl.read_id().ok();
        let usb_buffer_size = jl.usb_buffer_size().ok();
        let loader_version = jl.loader_version().ok().flatten();
        let maskrom_id = jl.maskrom_id().ok();
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&json!({
                "device": resolved.selector,
                "transport_path": resolved.transport_path,
                "protocol": format!("{:?}", ctx.protocol),
                "inquiry": {
                    "vendor": inquiry.vendor,
                    "product": inquiry.product,
                    "revision": inquiry.revision,
                },
                "online_device": online.map(|i| json!({"kind": format!("{:?}", i.kind), "id": i.id})),
                "flash_id": flash_id,
                "usb_buffer_size": usb_buffer_size,
                "loader_version": loader_version,
                "maskrom_id": maskrom_id,
            }))?
        )?;
        return Ok(());
    }
    writeln!(out, "device          : {}", resolved.selector)?;
    writeln!(out, "vendor          : {}", inquiry.vendor)?;
    writeln!(out, "product         : {}", inquiry.product)?;
    writeln!(out, "revision        : {}", inquiry.revision)?;
    writeln!(out, "protocol        : {:?}", ctx.protocol)?;
    match jl.online_device() {
        Ok(info) => writeln!(out, "online device   : {:?}, id: {:?}", info.kind, info.id)?,
        Err(err) => writeln!(out, "online device   : <unavailable> ({err})")?,
    }
    match jl.read_id() {
        Ok(id) => writeln!(out, "flash id        : 0x{id:06x}")?,
        Err(err) => writeln!(out, "flash id        : <unavailable> ({err})")?,
    }
    match jl.usb_buffer_size() {
        Ok(size) => writeln!(out, "usb buffer size : {size}")?,
        Err(err) => writeln!(out, "usb buffer size : <unavailable> ({err})")?,
    }
    match jl.loader_version() {
        Ok(version) => writeln!(
            out,
            "loader version  : {}",
            version.as_deref().unwrap_or("<unknown>")
        )?,
        Err(err) => writeln!(out, "loader version  : <unavailable> ({err})")?,
    }
    match jl.maskrom_id() {
        Ok(id) => writeln!(out, "maskrom id      : 0x{id:08x}")?,
        Err(err) => writeln!(out, "maskrom id      : <unavailable> ({err})")?,
    }
    Ok(())
}

fn cmd_flash_read(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: RunCtx,
    req: ReadRequest<'_>,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, req.device, ctx.timeout_ms, ctx.protocol)?;
    let data = read_flash_chunked(
        jl.as_mut(),
        err,
        req.address,
        req.length,
        ctx.chunk_size,
        ctx.progress,
    )?;
    emit_read_output(out, data.as_slice(), req.address, req.output, "flash")?;
    Ok(())
}

fn cmd_flash_write(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    input: &str,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "flash-write")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let data = std::fs::read(input)?;
    write_flash_chunked(
        jl.as_mut(),
        err,
        address,
        &data,
        ctx.chunk_size,
        ctx.progress,
    )?;
    if ctx.json_mode {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true, "op": "flash-write", "address": address, "bytes": data.len(), "input": input
            }))?
        )?;
    } else {
        writeln!(
            out,
            "wrote {} bytes from {} into flash at 0x{address:08x}",
            data.len(),
            input
        )?;
    }
    Ok(())
}

fn cmd_flash_erase_sector(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "flash-erase-sector")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.flash_erase_sector(address)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "flash-erase-sector", "address": address}),
        &format!("erased flash sector at 0x{address:08x}"),
    )?;
    Ok(())
}

fn cmd_flash_erase_block(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "flash-erase-block")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.flash_erase_block(address)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "flash-erase-block", "address": address}),
        &format!("erased flash block at 0x{address:08x}"),
    )?;
    Ok(())
}

fn cmd_flash_erase_chip(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "flash-erase-chip")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.flash_erase_chip()?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "flash-erase-chip"}),
        "erased flash chip",
    )?;
    Ok(())
}

fn cmd_read_id(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let id = jl.read_id()?;
    print_query(
        out,
        ctx.json_mode,
        json!({"flash_id": id}),
        &format!("flash id: 0x{id:06x}"),
    )?;
    Ok(())
}

fn cmd_online_device(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let info = jl.online_device()?;
    print_query(
        out,
        ctx.json_mode,
        json!({"kind": format!("{:?}", info.kind), "id": info.id}),
        &format!("online device: {:?}, id: {:?}", info.kind, info.id),
    )?;
    Ok(())
}

fn cmd_usb_buffer_size(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let size = jl.usb_buffer_size()?;
    print_query(
        out,
        ctx.json_mode,
        json!({"usb_buffer_size": size}),
        &format!("usb buffer size: {size}"),
    )?;
    Ok(())
}

fn cmd_version(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let version = jl.loader_version()?;
    let text = version.as_deref().unwrap_or("<unknown>");
    print_query(
        out,
        ctx.json_mode,
        json!({"loader_version": version}),
        &format!("loader version: {text}"),
    )?;
    Ok(())
}

fn cmd_maskrom_id(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let id = jl.maskrom_id()?;
    print_query(
        out,
        ctx.json_mode,
        json!({"maskrom_id": id}),
        &format!("maskrom id: 0x{id:08x}"),
    )?;
    Ok(())
}

fn cmd_read_status(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let status = jl.read_status()?;
    print_query(
        out,
        ctx.json_mode,
        json!({"status": status}),
        &format!("status: 0x{status:02x}"),
    )?;
    Ok(())
}

fn cmd_flash_crc16(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    length: usize,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let crc = jl.flash_crc16(address, length)?;
    print_query(
        out,
        ctx.json_mode,
        json!({"flash_crc16": crc, "address": address, "length": length}),
        &format!("flash crc16: 0x{crc:04x}"),
    )?;
    Ok(())
}

fn cmd_flash_crc16_raw(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    length: usize,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let crc = jl.flash_crc16_raw(address, length)?;
    print_query(
        out,
        ctx.json_mode,
        json!({"flash_crc16_raw": crc, "address": address, "length": length}),
        &format!("flash crc16 raw: 0x{crc:04x}"),
    )?;
    Ok(())
}

fn cmd_set_flash_cmds(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    cmds: &[u8],
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "set-flash-cmds")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.set_flash_cmds(cmds)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "set-flash-cmds", "cmds_len": cmds.len()}),
        &format!("set flash cmds: {} bytes", cmds.len()),
    )?;
    Ok(())
}

fn cmd_chip_key(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    arg: u32,
    raw: bool,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    if raw {
        let raw = jl.read_key_raw(arg)?;
        if ctx.json_mode {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&json!({"chipkey_raw": raw.payload}))?
            )?;
        } else {
            write!(out, "chipkey raw:")?;
            for b in raw.payload {
                write!(out, " {b:02x}")?;
            }
            writeln!(out)?;
        }
    } else {
        let key = jl.chip_key(arg)?;
        print_query(
            out,
            ctx.json_mode,
            json!({"chipkey": key}),
            &format!("chipkey: 0x{key:04x}"),
        )?;
    }
    Ok(())
}

fn cmd_write_chip_key(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    key: u32,
    vpp: u32,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "write-chip-key")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let result = jl.write_chipkey(key, vpp)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "write-chip-key", "result": result}),
        &format!("write-chipkey result: 0x{result:08x}"),
    )?;
    Ok(())
}

fn cmd_flash_select(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    kind: FlashSelectKind,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.flash_select(kind.into())?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "flash-select", "kind": format!("{:?}", kind)}),
        &format!("flash-select: {:?}", kind),
    )?;
    Ok(())
}

fn cmd_mem_read(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: RunCtx,
    req: ReadRequest<'_>,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, req.device, ctx.timeout_ms, ctx.protocol)?;
    let data = read_memory_chunked(
        jl.as_mut(),
        err,
        req.address,
        req.length,
        ctx.chunk_size,
        ctx.progress,
    )?;
    emit_read_output(out, data.as_slice(), req.address, req.output, "memory")?;
    Ok(())
}

fn cmd_mem_write(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    input: &str,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let data = std::fs::read(input)?;
    write_memory_chunked(
        jl.as_mut(),
        err,
        address,
        &data,
        ctx.chunk_size,
        ctx.progress,
    )?;
    if ctx.json_mode {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(
                &json!({"ok": true, "op": "mem-write", "address": address, "bytes": data.len(), "input": input})
            )?
        )?;
    } else {
        writeln!(
            out,
            "wrote {} bytes from {} into memory at 0x{address:08x}",
            data.len(),
            input
        )?;
    }
    Ok(())
}

fn cmd_mem_write_rxgp(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    err: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    input: &str,
) -> Result<(), BoxError> {
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    let data = std::fs::read(input)?;
    write_memory_rxgp_chunked(
        jl.as_mut(),
        err,
        address,
        &data,
        ctx.chunk_size,
        ctx.progress,
    )?;
    if ctx.json_mode {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(
                &json!({"ok": true, "op": "mem-write-rxgp", "address": address, "bytes": data.len(), "input": input})
            )?
        )?;
    } else {
        writeln!(
            out,
            "wrote {} bytes from {} into memory (rxgp) at 0x{address:08x}",
            data.len(),
            input
        )?;
    }
    Ok(())
}

fn cmd_jump(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    address: u32,
    arg: u32,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "jump")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.mem_jump(address, arg)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "jump", "address": address, "arg": arg}),
        &format!("jumped to 0x{address:08x} arg=0x{arg:08x}"),
    )?;
    Ok(())
}

fn cmd_run_app(
    factory: &dyn DeviceFactory,
    out: &mut dyn Write,
    device: Option<&str>,
    ctx: RunCtx,
    arg: u32,
) -> Result<(), BoxError> {
    require_yes(ctx.yes, "run-app")?;
    let (_, mut jl) = open_device(factory, device, ctx.timeout_ms, ctx.protocol)?;
    jl.run_app(arg)?;
    print_action(
        out,
        ctx.json_mode,
        json!({"ok": true, "op": "run-app", "arg": arg}),
        &format!("run-app arg=0x{arg:08x}"),
    )?;
    Ok(())
}

fn print_query(
    out: &mut dyn Write,
    json_mode: bool,
    value: serde_json::Value,
    text: &str,
) -> Result<(), BoxError> {
    if json_mode {
        writeln!(out, "{}", serde_json::to_string_pretty(&value)?)?;
    } else {
        writeln!(out, "{text}")?;
    }
    Ok(())
}

fn print_action(
    out: &mut dyn Write,
    json_mode: bool,
    value: serde_json::Value,
    text: &str,
) -> Result<(), BoxError> {
    print_query(out, json_mode, value, text)
}

fn emit_read_output(
    out: &mut dyn Write,
    data: &[u8],
    address: u32,
    opts: ReadOutputOpts<'_>,
    space: &str,
) -> Result<(), BoxError> {
    if let Some(path) = opts.output {
        std::fs::write(path, data)?;
        writeln!(
            out,
            "read {} bytes from {space} 0x{address:08x} into {}",
            data.len(),
            path
        )?;
        return Ok(());
    }
    if opts.stdout {
        out.write_all(data)?;
        return Ok(());
    }
    if opts.hexdump || (!opts.stdout && opts.output.is_none()) {
        print_hexdump(out, data, address)?;
    }
    Ok(())
}

fn print_hexdump(out: &mut dyn Write, data: &[u8], base: u32) -> Result<(), std::io::Error> {
    for (row, chunk) in data.chunks(16).enumerate() {
        let addr = base.wrapping_add((row * 16) as u32);
        write!(out, "{addr:08x}  ")?;
        for i in 0..16 {
            if i < chunk.len() {
                write!(out, "{:02x} ", chunk[i])?;
            } else {
                write!(out, "   ")?;
            }
        }
        write!(out, " ")?;
        for &b in chunk {
            let ch = if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                '.'
            };
            write!(out, "{ch}")?;
        }
        writeln!(out)?;
    }
    Ok(())
}

fn read_flash_chunked(
    jl: &mut dyn DeviceOps,
    err: &mut dyn Write,
    address: u32,
    length: usize,
    chunk_size: usize,
    progress: bool,
) -> Result<Vec<u8>, BoxError> {
    let mut out = Vec::with_capacity(length);
    let chunk_size = chunk_size.max(1);
    let mut offset = 0usize;
    while offset < length {
        let this_len = (length - offset).min(chunk_size);
        let addr = address.wrapping_add(offset as u32);
        let chunk = jl.flash_read(addr, this_len)?;
        out.extend_from_slice(&chunk);
        offset += this_len;
        progress_step(err, progress, "flash-read", offset, length);
    }
    Ok(out)
}

fn write_flash_chunked(
    jl: &mut dyn DeviceOps,
    err: &mut dyn Write,
    address: u32,
    data: &[u8],
    chunk_size: usize,
    progress: bool,
) -> Result<(), BoxError> {
    let chunk_size = chunk_size.max(1);
    for (idx, chunk) in data.chunks(chunk_size).enumerate() {
        let addr = address.wrapping_add((idx * chunk_size) as u32);
        jl.flash_write(addr, chunk)?;
        let done = ((idx + 1) * chunk_size).min(data.len());
        progress_step(err, progress, "flash-write", done, data.len());
    }
    Ok(())
}

fn read_memory_chunked(
    jl: &mut dyn DeviceOps,
    err: &mut dyn Write,
    address: u32,
    length: usize,
    chunk_size: usize,
    progress: bool,
) -> Result<Vec<u8>, BoxError> {
    let mut out = Vec::with_capacity(length);
    let chunk_size = chunk_size.max(1);
    let mut offset = 0usize;
    while offset < length {
        let this_len = (length - offset).min(chunk_size);
        let addr = address.wrapping_add(offset as u32);
        let chunk = jl.mem_read(addr, this_len)?;
        out.extend_from_slice(&chunk);
        offset += this_len;
        progress_step(err, progress, "mem-read", offset, length);
    }
    Ok(out)
}

fn write_memory_chunked(
    jl: &mut dyn DeviceOps,
    err: &mut dyn Write,
    address: u32,
    data: &[u8],
    chunk_size: usize,
    progress: bool,
) -> Result<(), BoxError> {
    let chunk_size = chunk_size.max(1);
    for (idx, chunk) in data.chunks(chunk_size).enumerate() {
        let addr = address.wrapping_add((idx * chunk_size) as u32);
        jl.mem_write(addr, chunk)?;
        let done = ((idx + 1) * chunk_size).min(data.len());
        progress_step(err, progress, "mem-write", done, data.len());
    }
    Ok(())
}

fn write_memory_rxgp_chunked(
    jl: &mut dyn DeviceOps,
    err: &mut dyn Write,
    address: u32,
    data: &[u8],
    chunk_size: usize,
    progress: bool,
) -> Result<(), BoxError> {
    let chunk_size = chunk_size.max(1);
    for (idx, chunk) in data.chunks(chunk_size).enumerate() {
        let addr = address.wrapping_add((idx * chunk_size) as u32);
        jl.mem_write_rxgp(addr, chunk)?;
        let done = ((idx + 1) * chunk_size).min(data.len());
        progress_step(err, progress, "mem-write-rxgp", done, data.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jl_sg::TransportError;
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeFactory {
        device: Option<FakeDevice>,
        err: Option<FakeOpenError>,
    }

    enum FakeOpenError {
        SgIo(String),
    }

    impl DeviceFactory for FakeFactory {
        fn open(
            &self,
            _device: &str,
            _timeout_ms: u32,
            _protocol: ProtocolKind,
        ) -> Result<Box<dyn DeviceOps>, BoxError> {
            if let Some(err) = &self.err {
                return match err {
                    FakeOpenError::SgIo(msg) => Err(Box::new(TransportError::SgIo(msg.clone()))),
                };
            }
            Ok(Box::new(self.device.clone().expect("fake device")))
        }
    }

    #[derive(Clone, Default)]
    struct FakeDevice {
        inquiry: Option<InquiryData>,
        online: Option<OnlineDeviceInfo>,
        read_id: Option<u32>,
        usb_buffer_size: Option<u32>,
        loader_version: Option<Option<String>>,
        maskrom_id: Option<u32>,
        flash_read: Option<Vec<u8>>,
        mem_read: Option<Vec<u8>>,
    }

    impl DeviceOps for FakeDevice {
        fn inquiry(&mut self) -> Result<InquiryData, UbootError> {
            Ok(self.inquiry.clone().unwrap())
        }
        fn online_device(&mut self) -> Result<OnlineDeviceInfo, UbootError> {
            Ok(self.online.clone().unwrap())
        }
        fn read_id(&mut self) -> Result<u32, UbootError> {
            Ok(self.read_id.unwrap())
        }
        fn usb_buffer_size(&mut self) -> Result<u32, UbootError> {
            Ok(self.usb_buffer_size.unwrap())
        }
        fn loader_version(&mut self) -> Result<Option<String>, UbootError> {
            Ok(self.loader_version.clone().unwrap())
        }
        fn maskrom_id(&mut self) -> Result<u32, UbootError> {
            Ok(self.maskrom_id.unwrap())
        }
        fn read_status(&mut self) -> Result<u8, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn flash_crc16(&mut self, _addr: u32, _len: usize) -> Result<u16, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn flash_crc16_raw(&mut self, _addr: u32, _len: usize) -> Result<u16, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn set_flash_cmds(&mut self, _cmds: &[u8]) -> Result<(), UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn read_key_raw(&mut self, _arg: u32) -> Result<ChipKeyRaw, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn chip_key(&mut self, _arg: u32) -> Result<u16, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn write_chipkey(&mut self, _key: u32, _vpp: u32) -> Result<u32, UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn flash_select(&mut self, _sel: LoaderV1FlashSelect) -> Result<(), UbootError> {
            Err(UbootError::UnsupportedProtocol)
        }
        fn flash_read(&mut self, _addr: u32, _len: usize) -> Result<Vec<u8>, UbootError> {
            Ok(self.flash_read.clone().unwrap())
        }
        fn flash_write(&mut self, _addr: u32, _data: &[u8]) -> Result<(), UbootError> {
            Ok(())
        }
        fn flash_erase_sector(&mut self, _addr: u32) -> Result<(), UbootError> {
            Ok(())
        }
        fn flash_erase_block(&mut self, _addr: u32) -> Result<(), UbootError> {
            Ok(())
        }
        fn flash_erase_chip(&mut self) -> Result<(), UbootError> {
            Ok(())
        }
        fn mem_read(&mut self, _addr: u32, _len: usize) -> Result<Vec<u8>, UbootError> {
            Ok(self.mem_read.clone().unwrap())
        }
        fn mem_write(&mut self, _addr: u32, _data: &[u8]) -> Result<(), UbootError> {
            Ok(())
        }
        fn mem_write_rxgp(&mut self, _addr: u32, _data: &[u8]) -> Result<(), UbootError> {
            Ok(())
        }
        fn mem_jump(&mut self, _addr: u32, _arg: u32) -> Result<(), UbootError> {
            Ok(())
        }
        fn run_app(&mut self, _arg: u32) -> Result<(), UbootError> {
            Ok(())
        }
    }

    fn fake_linux_selector() -> String {
        let tmp = tempdir().unwrap();
        let path = tmp.keep().join("sg3");
        std::fs::write(&path, []).unwrap();
        path.display().to_string()
    }

    #[test]
    fn probe_json_success_uses_fake_transport() {
        let selector = fake_linux_selector();
        let cli = Cli::parse_from(["jluboot", "--json", "probe", "--device", &selector]);
        let factory = FakeFactory {
            device: Some(FakeDevice {
                inquiry: Some(InquiryData {
                    vendor: "JIELI".into(),
                    product: "UBOOT".into(),
                    revision: "1.0".into(),
                    raw: vec![],
                }),
                online: Some(OnlineDeviceInfo {
                    kind: jl_uboot::OnlineDeviceType::Spi0Nor,
                    id: Some(1),
                }),
                read_id: Some(0x123456),
                usb_buffer_size: Some(4096),
                loader_version: Some(Some("L2A1".into())),
                maskrom_id: Some(0xdeadbeef),
                ..Default::default()
            }),
            err: None,
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();
        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 0);
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("\"vendor\": \"JIELI\""));
        assert!(stdout.contains("\"flash_id\": 1193046"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn find_json_entry_includes_candidate_metadata_fields() {
        let candidate = ScsiCandidate {
            selector: Some("/dev/sg3".into()),
            path: "/dev/sg3".into(),
            vid: Some(0x4A4C),
            pid: Some(0x00F1),
            note: Some("USBSTOR\\DISK&VEN_JL".into()),
        };
        let inquiry = InquiryData {
            vendor: "JIELI".into(),
            product: "UBOOT".into(),
            revision: "1.0".into(),
            raw: vec![],
        };
        let value = build_find_json_entry(
            &candidate,
            &inquiry,
            Some(Some("Spi0Nor".into())),
            Some(Some(0x123456)),
            Some(Some("L2A1".into())),
        );
        assert_eq!(value["selector"], json!("/dev/sg3"));
        assert_eq!(value["usb_vid"], json!(0x4A4C));
        assert_eq!(value["usb_pid"], json!(0x00F1));
        assert_eq!(value["note"], json!("USBSTOR\\DISK&VEN_JL"));
        assert_eq!(value["vendor"], json!("JIELI"));
        assert_eq!(value["product"], json!("UBOOT"));
        assert_eq!(value["online_device"], json!("Spi0Nor"));
        assert_eq!(value["flash_id"], json!(0x123456));
        assert_eq!(value["loader_version"], json!("L2A1"));
    }

    #[test]
    fn find_probe_json_success_includes_probe_and_candidate_metadata() {
        let factory = FakeFactory {
            device: Some(FakeDevice {
                inquiry: Some(InquiryData {
                    vendor: "JIELI".into(),
                    product: "UBOOT".into(),
                    revision: "1.0".into(),
                    raw: vec![],
                }),
                online: Some(OnlineDeviceInfo {
                    kind: jl_uboot::OnlineDeviceType::Spi0Nor,
                    id: Some(7),
                }),
                read_id: Some(0x123456),
                loader_version: Some(Some("L2A1".into())),
                ..Default::default()
            }),
            err: None,
        };
        let candidate = ScsiCandidate {
            selector: Some("/dev/sg3".into()),
            path: "/dev/sg3".into(),
            vid: Some(0x4A4C),
            pid: Some(0x00F1),
            note: Some("USBSTOR\\DISK&VEN_JL".into()),
        };
        let mut out = Vec::new();
        let mut json_entries = Vec::new();
        let mut dev = Box::new(factory.device.clone().unwrap()) as Box<dyn DeviceOps>;
        let inquiry = dev.inquiry().unwrap();
        let online = Some(format!("{:?}", dev.online_device().unwrap().kind));
        let flash_id = Some(dev.read_id().unwrap());
        let loader_version = dev.loader_version().unwrap();
        json_entries.push(build_find_json_entry(
            &candidate,
            &inquiry,
            Some(online),
            Some(flash_id),
            Some(loader_version),
        ));
        writeln!(
            &mut out,
            "{}",
            serde_json::to_string_pretty(&json_entries).unwrap()
        )
        .unwrap();

        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("\"selector\": \"/dev/sg3\""));
        assert!(stdout.contains("\"usb_vid\": 19020"));
        assert!(stdout.contains("\"usb_pid\": 241"));
        assert!(stdout.contains("\"note\": \"USBSTOR\\\\DISK&VEN_JL\""));
        assert!(stdout.contains("\"online_device\": \"Spi0Nor\""));
        assert!(stdout.contains("\"flash_id\": 1193046"));
        assert!(stdout.contains("\"loader_version\": \"L2A1\""));
    }

    #[test]
    fn flash_read_hexdump_success_uses_fake_transport() {
        let selector = fake_linux_selector();
        let cli = Cli::parse_from([
            "jluboot",
            "flash-read",
            "--device",
            &selector,
            "--address",
            "4096",
            "--length",
            "4",
            "--hexdump",
        ]);
        let factory = FakeFactory {
            device: Some(FakeDevice {
                flash_read: Some(vec![0x41, 0x42, 0x7f, 0x00]),
                ..Default::default()
            }),
            err: None,
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();
        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 0);
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("00001000"));
        assert!(stdout.contains("41 42 7f 00"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn progress_goes_to_stderr_only() {
        let selector = fake_linux_selector();
        let cli = Cli::parse_from([
            "jluboot",
            "--progress",
            "mem-read",
            "--device",
            &selector,
            "--address",
            "0",
            "--length",
            "4",
            "--stdout",
        ]);
        let factory = FakeFactory {
            device: Some(FakeDevice {
                mem_read: Some(vec![1, 2, 3, 4]),
                ..Default::default()
            }),
            err: None,
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();
        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 0);
        assert_eq!(out, vec![1, 2, 3, 4]);
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("progress mem-read"));
    }

    #[test]
    fn transport_errors_map_to_exit_code_12() {
        let selector = fake_linux_selector();
        let cli = Cli::parse_from(["jluboot", "read-id", "--device", &selector]);
        let factory = FakeFactory {
            device: None,
            err: Some(FakeOpenError::SgIo("boom".into())),
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();
        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 12);
        assert!(String::from_utf8(stderr).unwrap().contains("boom"));
    }

    #[test]
    fn protocol_failures_map_to_exit_code_13() {
        let selector = fake_linux_selector();
        let cli = Cli::parse_from(["jluboot", "read-status", "--device", &selector]);
        let factory = FakeFactory {
            device: Some(FakeDevice::default()),
            err: None,
        };
        let mut out = Vec::new();
        let mut stderr = Vec::new();
        let rc = run_cli_with_factory(&cli, &factory, &mut out, &mut stderr);
        assert_eq!(rc, 13);
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("unsupported protocol")
        );
    }

    #[test]
    fn selector_for_candidate_prefers_selector_then_note_then_path() {
        let candidate = ScsiCandidate {
            selector: Some("E:".into()),
            path: "/dev/sg3".into(),
            vid: None,
            pid: None,
            note: Some("USBSTOR".into()),
        };
        assert_eq!(selector_for_candidate(&candidate), "E:");

        let candidate = ScsiCandidate {
            selector: None,
            path: "/dev/sg3".into(),
            vid: None,
            pid: None,
            note: Some("USBSTOR".into()),
        };
        assert_eq!(selector_for_candidate(&candidate), "USBSTOR");

        let candidate = ScsiCandidate {
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

        let single = resolve_auto_detect_matches(vec![ScsiCandidate {
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
            ScsiCandidate {
                selector: Some("E:".into()),
                path: "/dev/sg3".into(),
                vid: None,
                pid: None,
                note: None,
            },
            ScsiCandidate {
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
                ScsiCandidate {
                    selector: Some("not-jieli".into()),
                    path: "/dev/sg2".into(),
                    vid: None,
                    pid: None,
                    note: None,
                },
                ScsiCandidate {
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
