use std::ffi::c_void;
use std::fs::{File, read_dir};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use libc::{c_int, c_short, c_uchar, c_uint, c_ushort};

use crate::{InquiryData, ScsiResponse, ScsiTransport, TransportError};

pub const DEFAULT_SG_ROOT: &str = "/dev";
pub const DEFAULT_SYS_CLASS_BLOCK_ROOT: &str = "/sys/class/block";

const SG_IO: libc::c_ulong = 0x2285;
const SG_INTERFACE_ID_ORIG: c_int = b'S' as c_int;
const SG_DXFER_NONE: c_int = -1;
const SG_DXFER_TO_DEV: c_int = -2;
const SG_DXFER_FROM_DEV: c_int = -3;
const SG_INFO_OK_MASK: u32 = 0x1;
const SG_INFO_OK: u32 = 0x0;
const DEFAULT_TIMEOUT_MS: u32 = 5000;
const DEFAULT_SENSE_LEN: usize = 32;

#[repr(C)]
struct SgIoHdr {
    interface_id: c_int,
    dxfer_direction: c_int,
    cmd_len: c_uchar,
    mx_sb_len: c_uchar,
    iovec_count: c_short,
    dxfer_len: c_uint,
    dxferp: *mut c_void,
    cmdp: *mut c_uchar,
    sbp: *mut c_uchar,
    timeout: c_uint,
    flags: c_uint,
    pack_id: c_int,
    usr_ptr: *mut c_void,
    status: c_uchar,
    masked_status: c_uchar,
    msg_status: c_uchar,
    sb_len_wr: c_uchar,
    host_status: c_ushort,
    driver_status: c_ushort,
    resid: c_int,
    duration: c_uint,
    info: c_uint,
}

pub fn candidate_sg_paths() -> Result<Vec<PathBuf>, TransportError> {
    let mut paths = Vec::new();

    for entry in read_dir(DEFAULT_SG_ROOT)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("sg") {
            paths.push(entry.path());
        }
    }

    paths.sort();
    Ok(paths)
}

pub fn resolve_selector_to_sg_path(selector: impl AsRef<Path>) -> Result<PathBuf, TransportError> {
    resolve_selector_to_sg_path_with_roots(
        selector.as_ref(),
        Path::new(DEFAULT_SG_ROOT),
        Path::new(DEFAULT_SYS_CLASS_BLOCK_ROOT),
    )
}

fn resolve_selector_to_sg_path_with_roots(
    selector: &Path,
    dev_root: &Path,
    sys_block_root: &Path,
) -> Result<PathBuf, TransportError> {
    if !selector.exists() {
        return Err(TransportError::DeviceNotFound(
            selector.display().to_string(),
        ));
    }

    let canonical = selector
        .canonicalize()
        .unwrap_or_else(|_| selector.to_path_buf());
    let file_name = canonical
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| TransportError::DeviceNotFound(selector.display().to_string()))?;

    if file_name.starts_with("sg") {
        return Ok(canonical);
    }

    let sys_block = sys_block_root.join(file_name);
    if !sys_block.exists() {
        return Ok(canonical);
    }

    let scsi_generic_dir = sys_block.join("device").join("scsi_generic");
    if !scsi_generic_dir.exists() {
        return Err(TransportError::SgIo(format!(
            "no scsi_generic mapping found for {}",
            canonical.display()
        )));
    }

    let mut matches = Vec::new();
    for entry in read_dir(&scsi_generic_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("sg") {
            matches.push(dev_root.join(name.as_ref()));
        }
    }

    matches.sort();
    matches.dedup();

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(TransportError::SgIo(format!(
            "no scsi_generic device found for {}",
            canonical.display()
        ))),
        n => Err(TransportError::SgIo(format!(
            "multiple scsi_generic devices found for {} ({n})",
            canonical.display()
        ))),
    }
}

pub struct LinuxSgDevice {
    path: PathBuf,
    file: File,
    timeout_ms: u32,
}

impl LinuxSgDevice {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        Self::open_with_timeout(path, DEFAULT_TIMEOUT_MS)
    }

    pub fn open_with_timeout(
        path: impl AsRef<Path>,
        timeout_ms: u32,
    ) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(TransportError::DeviceNotFound(path.display().to_string()));
        }
        let file = File::options().read(true).write(true).open(&path)?;
        Ok(Self {
            path,
            file,
            timeout_ms,
        })
    }
}

impl ScsiTransport for LinuxSgDevice {
    fn path(&self) -> &Path {
        &self.path
    }

    fn exec(
        &mut self,
        cdb: &[u8],
        data_out: Option<&[u8]>,
        data_in_len: usize,
    ) -> Result<ScsiResponse, TransportError> {
        if cdb.is_empty() || cdb.len() > 255 {
            return Err(TransportError::InvalidCdbLength(cdb.len()));
        }

        if data_out.is_some() && data_in_len != 0 {
            return Err(TransportError::SgIo(
                "bidirectional SG_IO transfers are not supported".to_string(),
            ));
        }

        let mut cdb_buf = cdb.to_vec();
        let mut sense_buf = vec![0u8; DEFAULT_SENSE_LEN];
        let mut data_in = if data_in_len != 0 {
            vec![0u8; data_in_len]
        } else {
            Vec::new()
        };
        let mut data_out_buf = data_out.map(|d| d.to_vec());

        let (dxfer_direction, dxfer_len, dxferp) = match (data_out_buf.as_mut(), data_in_len) {
            (Some(out), 0) => (
                SG_DXFER_TO_DEV,
                out.len() as u32,
                out.as_mut_ptr().cast::<c_void>(),
            ),
            (None, len) if len > 0 => (
                SG_DXFER_FROM_DEV,
                len as u32,
                data_in.as_mut_ptr().cast::<c_void>(),
            ),
            _ => (SG_DXFER_NONE, 0, std::ptr::null_mut()),
        };

        let mut hdr = SgIoHdr {
            interface_id: SG_INTERFACE_ID_ORIG,
            dxfer_direction,
            cmd_len: cdb_buf.len() as c_uchar,
            mx_sb_len: sense_buf.len() as c_uchar,
            iovec_count: 0,
            dxfer_len,
            dxferp,
            cmdp: cdb_buf.as_mut_ptr(),
            sbp: sense_buf.as_mut_ptr(),
            timeout: self.timeout_ms,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        };

        let rc = unsafe { libc::ioctl(self.file.as_raw_fd(), SG_IO, &mut hdr) };
        if rc < 0 {
            return Err(TransportError::Io(std::io::Error::last_os_error()));
        }

        if (hdr.info & SG_INFO_OK_MASK) != SG_INFO_OK {
            let sense_len = usize::from(hdr.sb_len_wr).min(sense_buf.len());
            let sense_hex = if sense_len > 0 {
                hex_encode(&sense_buf[..sense_len])
            } else {
                String::new()
            };
            return Err(TransportError::SgIo(format!(
                "transfer failed: info=0x{:02x}, host_status=0x{:02x}, driver_status=0x{:02x}, sense={}",
                hdr.info, hdr.host_status, hdr.driver_status, sense_hex
            )));
        }

        let resid = if hdr.resid < 0 { 0 } else { hdr.resid as usize };
        let valid_len = data_in.len().saturating_sub(resid);
        data_in.truncate(valid_len);

        Ok(ScsiResponse {
            data: data_in,
            resid,
        })
    }

    fn inquiry(&mut self) -> Result<InquiryData, TransportError> {
        let cdb = [0x12_u8, 0x00, 0x00, 0x00, 36, 0x00];
        let response = self.exec(&cdb, None, 36)?;
        if response.data.len() < 36 {
            return Err(TransportError::InvalidInquiry);
        }

        let vendor = ascii_field(&response.data[8..16])?;
        let product = ascii_field(&response.data[16..32])?;
        let revision = ascii_field(&response.data[32..36])?;

        Ok(InquiryData {
            vendor,
            product,
            revision,
            raw: response.data,
        })
    }
}

fn ascii_field(buf: &[u8]) -> Result<String, TransportError> {
    let s = std::str::from_utf8(buf).map_err(|_| TransportError::InvalidInquiry)?;
    Ok(s.trim().to_string())
}

fn hex_encode(buf: &[u8]) -> String {
    let mut out = String::with_capacity(buf.len() * 2);
    for b in buf {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resolve_selector_to_sg_path_with_roots;
    use std::fs::{self, File};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn sg_selector_is_returned_directly() {
        let tmp = tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sys_root = tmp.path().join("sys/class/block");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(&sys_root).unwrap();
        let sg3 = dev_root.join("sg3");
        File::create(&sg3).unwrap();

        let resolved = resolve_selector_to_sg_path_with_roots(&sg3, &dev_root, &sys_root).unwrap();
        assert_eq!(resolved, sg3);
    }

    #[test]
    fn block_selector_maps_to_scsi_generic() {
        let tmp = tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let sys_root = tmp.path().join("sys/class/block");
        fs::create_dir_all(&dev_root).unwrap();
        fs::create_dir_all(sys_root.join("sdb/device/scsi_generic")).unwrap();
        let sdb = dev_root.join("sdb");
        let sg3 = dev_root.join("sg3");
        File::create(&sdb).unwrap();
        File::create(&sg3).unwrap();
        fs::create_dir_all(sys_root.join("sdb/device/scsi_generic/sg3")).unwrap();

        let resolved = resolve_selector_to_sg_path_with_roots(&sdb, &dev_root, &sys_root).unwrap();
        assert_eq!(resolved, sg3);
    }

    #[test]
    fn symlinked_block_selector_maps_to_scsi_generic() {
        let tmp = tempdir().unwrap();
        let dev_root = tmp.path().join("dev");
        let by_id = dev_root.join("disk/by-id");
        let sys_root = tmp.path().join("sys/class/block");
        fs::create_dir_all(&by_id).unwrap();
        fs::create_dir_all(sys_root.join("sdc/device/scsi_generic")).unwrap();
        let sdc = dev_root.join("sdc");
        let sg4 = dev_root.join("sg4");
        File::create(&sdc).unwrap();
        File::create(&sg4).unwrap();
        fs::create_dir_all(sys_root.join("sdc/device/scsi_generic/sg4")).unwrap();
        symlink(&sdc, by_id.join("usb-jieli-target")).unwrap();

        let resolved = resolve_selector_to_sg_path_with_roots(
            &by_id.join("usb-jieli-target"),
            &dev_root,
            &sys_root,
        )
        .unwrap();
        assert_eq!(resolved, sg4);
    }
}
