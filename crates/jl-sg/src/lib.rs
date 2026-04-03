pub mod linux;
pub mod windows;

use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScsiDirection {
    None,
    ToDevice,
    FromDevice,
}

#[derive(Debug, Clone)]
pub struct ScsiResponse {
    pub data: Vec<u8>,
    pub resid: usize,
}

#[derive(Debug, Clone)]
pub struct InquiryData {
    pub vendor: String,
    pub product: String,
    pub revision: String,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ScsiCandidate {
    pub selector: Option<String>,
    pub path: PathBuf,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub note: Option<String>,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("unsupported on this platform")]
    Unsupported,
    #[error("device path not found: {0}")]
    DeviceNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid cdb length: {0}")]
    InvalidCdbLength(usize),
    #[error("linux SG_IO transport error: {0}")]
    SgIo(String),
    #[error("windows SCSI transport error: {0}")]
    WinScsi(String),
    #[error("invalid inquiry response")]
    InvalidInquiry,
}

pub trait ScsiTransport {
    fn path(&self) -> &Path;

    fn exec(
        &mut self,
        cdb: &[u8],
        data_out: Option<&[u8]>,
        data_in_len: usize,
    ) -> Result<ScsiResponse, TransportError>;

    fn inquiry(&mut self) -> Result<InquiryData, TransportError>;
}

pub fn candidate_devices() -> Result<Vec<ScsiCandidate>, TransportError> {
    #[cfg(target_os = "linux")]
    {
        return Ok(linux::candidate_sg_paths()?
            .into_iter()
            .map(|path| ScsiCandidate {
                selector: Some(path.display().to_string()),
                path,
                vid: None,
                pid: None,
                note: None,
            })
            .collect());
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(windows::enumerate_usb_msc_candidates()?
            .into_iter()
            .map(|c| ScsiCandidate {
                selector: c.visible_selector,
                path: c.device_path,
                vid: c.vid,
                pid: c.pid,
                note: Some(c.note),
            })
            .collect());
    }
    #[allow(unreachable_code)]
    Err(TransportError::Unsupported)
}

pub fn candidate_device_paths() -> Result<Vec<PathBuf>, TransportError> {
    Ok(candidate_devices()?.into_iter().map(|c| c.path).collect())
}
