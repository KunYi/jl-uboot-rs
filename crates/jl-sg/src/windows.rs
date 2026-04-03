#[cfg(target_os = "windows")]
mod imp {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, GUID_DEVINTERFACE_DISK, HDEVINFO,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW,
    };
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    use crate::{InquiryData, ScsiResponse, ScsiTransport, TransportError};

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const IOCTL_SCSI_PASS_THROUGH_DIRECT: u32 = 0x0004_D014;
    const SCSI_IOCTL_DATA_OUT: u8 = 0;
    const SCSI_IOCTL_DATA_IN: u8 = 1;
    const SCSI_IOCTL_DATA_UNSPECIFIED: u8 = 2;
    const DEFAULT_TIMEOUT_MS: u32 = 5000;
    const DEFAULT_SENSE_LEN: usize = 32;
    const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002d1080;

    #[repr(C)]
    struct ScsiPassThroughDirect {
        length: u16,
        scsi_status: u8,
        path_id: u8,
        target_id: u8,
        lun: u8,
        cdb_length: u8,
        sense_info_length: u8,
        data_in: u8,
        data_transfer_length: u32,
        time_out_value: u32,
        data_buffer: *mut core::ffi::c_void,
        sense_info_offset: u32,
        cdb: [u8; 16],
    }

    #[repr(C)]
    struct SptdWithSense {
        sptd: ScsiPassThroughDirect,
        filler: u32,
        sense: [u8; DEFAULT_SENSE_LEN],
    }

    #[derive(Debug, Clone)]
    pub struct SetupApiUsbMscCandidate {
        pub device_path: PathBuf,
        pub vid: Option<u16>,
        pub pid: Option<u16>,
        pub interface_guid: [u8; 16],
        pub note: String,
        pub visible_selector: Option<String>,
    }

    #[repr(C)]
    struct StorageDeviceNumber {
        device_type: u32,
        device_number: u32,
        partition_number: u32,
    }

    pub const DEFAULT_USB_VID_ALLOWLIST: &[u16] = &[0x4A4C];

    pub fn enumerate_usb_msc_candidates_with_allowlist(
        allowlist: &[u16],
    ) -> Result<Vec<SetupApiUsbMscCandidate>, TransportError> {
        let devinfo = unsafe {
            SetupDiGetClassDevsW(
                &GUID_DEVINTERFACE_DISK,
                null(),
                0,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if devinfo == INVALID_HANDLE_VALUE as HDEVINFO {
            return Err(TransportError::Io(std::io::Error::last_os_error()));
        }

        let mut list = SetupDiList(devinfo);
        let mut out = Vec::new();
        let mut index = 0;

        loop {
            let mut if_data: SP_DEVICE_INTERFACE_DATA = unsafe { zeroed() };
            if_data.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;

            let ok = unsafe {
                SetupDiEnumDeviceInterfaces(
                    list.0,
                    null_mut(),
                    &GUID_DEVINTERFACE_DISK,
                    index,
                    &mut if_data,
                )
            };
            if ok == 0 {
                let err = std::io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(code) if code as u32 == ERROR_NO_MORE_ITEMS => break,
                    _ => return Err(TransportError::Io(err)),
                }
            }

            let mut devinfo_data: SP_DEVINFO_DATA = unsafe { zeroed() };
            devinfo_data.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;

            let path = match query_interface_path(list.0, &if_data, &mut devinfo_data) {
                Ok(path) => path,
                Err(_) => {
                    index += 1;
                    continue;
                }
            };

            let instance_id = match query_instance_id(list.0, &mut devinfo_data) {
                Ok(id) => id,
                Err(_) => {
                    index += 1;
                    continue;
                }
            };

            if !common::looks_like_usb_msc_instance(&instance_id) {
                index += 1;
                continue;
            }

            let (vid, pid) = common::parse_vid_pid(&instance_id);
            if !common::matches_allowlist(vid, allowlist) {
                index += 1;
                continue;
            }

            out.push(SetupApiUsbMscCandidate {
                device_path: path,
                vid,
                pid,
                interface_guid: guid_to_bytes(&GUID_DEVINTERFACE_DISK),
                note: instance_id,
                visible_selector: None,
            });

            index += 1;
        }

        Ok(attach_visible_selectors(out))
    }

    pub fn enumerate_usb_msc_candidates() -> Result<Vec<SetupApiUsbMscCandidate>, TransportError> {
        enumerate_usb_msc_candidates_with_allowlist(DEFAULT_USB_VID_ALLOWLIST)
    }

    pub fn candidate_windows_paths() -> Result<Vec<PathBuf>, TransportError> {
        let candidates = enumerate_usb_msc_candidates()?;
        Ok(candidates.into_iter().map(|c| c.device_path).collect())
    }

    pub fn resolve_visible_selector_to_candidate(
        selector: &str,
    ) -> Result<SetupApiUsbMscCandidate, TransportError> {
        let selector = normalize_volume_selector(selector)
            .ok_or_else(|| TransportError::DeviceNotFound(selector.to_string()))?;
        let volume_devnum = query_volume_selector_device_number(&selector)?;
        let candidates = enumerate_usb_msc_candidates()?;
        for candidate in candidates {
            let candidate_devnum = match query_disk_device_number(&candidate.device_path) {
                Ok(num) => num,
                Err(_) => continue,
            };
            if candidate_devnum == volume_devnum {
                let mut matched = candidate;
                matched.visible_selector = Some(selector.clone());
                return Ok(matched);
            }
        }
        Err(TransportError::DeviceNotFound(selector))
    }

    pub fn resolve_visible_selector_to_path(selector: &str) -> Result<PathBuf, TransportError> {
        Ok(resolve_visible_selector_to_candidate(selector)?.device_path)
    }

    pub struct WindowsScsiDevice {
        path: PathBuf,
        handle: HANDLE,
        timeout_ms: u32,
    }

    impl WindowsScsiDevice {
        pub fn open(path: impl AsRef<Path>) -> Result<Self, TransportError> {
            Self::open_with_timeout(path, DEFAULT_TIMEOUT_MS)
        }

        pub fn open_with_timeout(
            path: impl AsRef<Path>,
            timeout_ms: u32,
        ) -> Result<Self, TransportError> {
            let path = path.as_ref().to_path_buf();
            let wide = to_wide(&path);
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    0,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                let io = std::io::Error::last_os_error();
                return match io.kind() {
                    std::io::ErrorKind::NotFound => {
                        Err(TransportError::DeviceNotFound(path.display().to_string()))
                    }
                    _ => Err(TransportError::Io(io)),
                };
            }
            Ok(Self {
                path,
                handle,
                timeout_ms,
            })
        }

        pub fn timeout_ms(&self) -> u32 {
            self.timeout_ms
        }
    }

    impl Drop for WindowsScsiDevice {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    impl ScsiTransport for WindowsScsiDevice {
        fn path(&self) -> &Path {
            &self.path
        }

        fn exec(
            &mut self,
            cdb: &[u8],
            data_out: Option<&[u8]>,
            data_in_len: usize,
        ) -> Result<ScsiResponse, TransportError> {
            if cdb.is_empty() || cdb.len() > 16 {
                return Err(TransportError::InvalidCdbLength(cdb.len()));
            }
            if data_out.is_some() && data_in_len != 0 {
                return Err(TransportError::WinScsi(
                    "bidirectional SCSI pass-through is not supported".to_string(),
                ));
            }

            let mut data_in = if data_in_len > 0 {
                vec![0u8; data_in_len]
            } else {
                Vec::new()
            };
            let mut data_out_buf = data_out.map(|d| d.to_vec());
            let (data_in_mode, data_len, data_ptr) = match (&mut data_out_buf, data_in_len) {
                (Some(out), 0) => (
                    SCSI_IOCTL_DATA_OUT,
                    out.len() as u32,
                    out.as_mut_ptr().cast::<core::ffi::c_void>(),
                ),
                (None, len) if len > 0 => (
                    SCSI_IOCTL_DATA_IN,
                    len as u32,
                    data_in.as_mut_ptr().cast::<core::ffi::c_void>(),
                ),
                _ => (SCSI_IOCTL_DATA_UNSPECIFIED, 0, null_mut()),
            };

            let mut pkt: SptdWithSense = unsafe { zeroed() };
            pkt.sptd.length = size_of::<ScsiPassThroughDirect>() as u16;
            pkt.sptd.cdb_length = cdb.len() as u8;
            pkt.sptd.sense_info_length = DEFAULT_SENSE_LEN as u8;
            pkt.sptd.data_in = data_in_mode;
            pkt.sptd.data_transfer_length = data_len;
            pkt.sptd.time_out_value = timeout_secs(self.timeout_ms);
            pkt.sptd.data_buffer = data_ptr;
            pkt.sptd.sense_info_offset =
                (size_of::<ScsiPassThroughDirect>() + size_of::<u32>()) as u32;
            pkt.sptd.cdb[..cdb.len()].copy_from_slice(cdb);

            let mut bytes_returned = 0u32;
            let ok = unsafe {
                DeviceIoControl(
                    self.handle,
                    IOCTL_SCSI_PASS_THROUGH_DIRECT,
                    (&mut pkt as *mut SptdWithSense).cast(),
                    size_of::<SptdWithSense>() as u32,
                    (&mut pkt as *mut SptdWithSense).cast(),
                    size_of::<SptdWithSense>() as u32,
                    &mut bytes_returned,
                    null_mut(),
                )
            };
            if ok == 0 {
                return Err(TransportError::Io(std::io::Error::last_os_error()));
            }

            if pkt.sptd.scsi_status != 0 {
                let sense_len = usize::from(pkt.sptd.sense_info_length).min(DEFAULT_SENSE_LEN);
                let sense_hex = hex_encode(&pkt.sense[..sense_len]);
                return Err(TransportError::WinScsi(format!(
                    "sptd failed: scsi_status=0x{:02x}, sense={sense_hex}",
                    pkt.sptd.scsi_status
                )));
            }

            Ok(ScsiResponse {
                data: data_in,
                resid: 0,
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

    fn timeout_secs(timeout_ms: u32) -> u32 {
        timeout_ms.div_ceil(1000).max(1)
    }

    fn attach_visible_selectors(
        mut candidates: Vec<SetupApiUsbMscCandidate>,
    ) -> Vec<SetupApiUsbMscCandidate> {
        let selector_map = collect_visible_selectors_by_device_number();
        for candidate in &mut candidates {
            let Ok(device_number) = query_disk_device_number(&candidate.device_path) else {
                continue;
            };
            if let Some(selector) = selector_map.get(&device_number) {
                candidate.visible_selector = Some(selector.clone());
            }
        }
        candidates
    }

    fn collect_visible_selectors_by_device_number() -> HashMap<u32, String> {
        let mut out = HashMap::new();
        for letter in b'A'..=b'Z' {
            let selector = format!("{}:", letter as char);
            let Ok(device_number) = query_volume_selector_device_number(&selector) else {
                continue;
            };
            out.entry(device_number).or_insert(selector);
        }
        out
    }

    fn normalize_volume_selector(selector: &str) -> Option<String> {
        let bytes = selector.as_bytes();
        if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Some(format!("{}:", (bytes[0] as char).to_ascii_uppercase()));
        }
        None
    }

    struct SetupDiList(HDEVINFO);

    impl Drop for SetupDiList {
        fn drop(&mut self) {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }

    fn query_interface_path(
        devinfo: HDEVINFO,
        if_data: &SP_DEVICE_INTERFACE_DATA,
        devinfo_data: &mut SP_DEVINFO_DATA,
    ) -> Result<PathBuf, TransportError> {
        let mut needed = 0u32;
        let ok = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                devinfo,
                if_data,
                null_mut(),
                0,
                &mut needed,
                devinfo_data,
            )
        };
        if ok != 0 {
            return Err(TransportError::WinScsi(
                "unexpected SetupDiGetDeviceInterfaceDetailW success with zero-sized buffer"
                    .to_string(),
            ));
        }

        let err = std::io::Error::last_os_error();
        if err.raw_os_error().map(|e| e as u32) != Some(ERROR_INSUFFICIENT_BUFFER) || needed == 0 {
            return Err(TransportError::Io(err));
        }

        let mut buf = vec![0u8; needed as usize];
        let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        unsafe {
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        }

        let ok = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                devinfo,
                if_data,
                detail,
                needed,
                &mut needed,
                devinfo_data,
            )
        };
        if ok == 0 {
            return Err(TransportError::Io(std::io::Error::last_os_error()));
        }

        let device_path = unsafe { widestr_ptr_to_string((*detail).DevicePath.as_ptr())? };
        Ok(PathBuf::from(device_path))
    }

    fn query_instance_id(
        devinfo: HDEVINFO,
        devinfo_data: &mut SP_DEVINFO_DATA,
    ) -> Result<String, TransportError> {
        let mut buf = vec![0u16; 512];
        let mut needed = 0u32;
        let ok = unsafe {
            SetupDiGetDeviceInstanceIdW(
                devinfo,
                devinfo_data,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(TransportError::Io(std::io::Error::last_os_error()));
        }
        widestr_to_string(&buf)
    }

    fn widestr_ptr_to_string(ptr: *const u16) -> Result<String, TransportError> {
        let mut len = 0usize;
        loop {
            let ch = unsafe { *ptr.add(len) };
            if ch == 0 {
                break;
            }
            len += 1;
        }
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf16(slice).map_err(|_| {
            TransportError::WinScsi("failed to decode UTF-16 device interface path".to_string())
        })
    }

    fn widestr_to_string(buf: &[u16]) -> Result<String, TransportError> {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16(&buf[..len]).map_err(|_| {
            TransportError::WinScsi("failed to decode UTF-16 device instance id".to_string())
        })
    }

    fn raw_volume_path(selector: &str) -> Result<PathBuf, TransportError> {
        let selector = normalize_volume_selector(selector)
            .ok_or_else(|| TransportError::DeviceNotFound(selector.to_string()))?;
        Ok(PathBuf::from(format!(r"\\.\{selector}")))
    }

    fn query_volume_selector_device_number(selector: &str) -> Result<u32, TransportError> {
        let volume_path = raw_volume_path(selector)?;
        query_storage_device_number(&volume_path)
    }

    fn query_disk_device_number(path: &Path) -> Result<u32, TransportError> {
        query_storage_device_number(path)
    }

    fn query_storage_device_number(path: &Path) -> Result<u32, TransportError> {
        let wide = to_wide(path);
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                0,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let io = std::io::Error::last_os_error();
            return match io.kind() {
                std::io::ErrorKind::NotFound => {
                    Err(TransportError::DeviceNotFound(path.display().to_string()))
                }
                _ => Err(TransportError::Io(io)),
            };
        }

        let mut sdn: StorageDeviceNumber = unsafe { zeroed() };
        let mut bytes_returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                null_mut(),
                0,
                (&mut sdn as *mut StorageDeviceNumber).cast(),
                size_of::<StorageDeviceNumber>() as u32,
                &mut bytes_returned,
                null_mut(),
            )
        };
        unsafe {
            let _ = CloseHandle(handle);
        }
        if ok == 0 {
            return Err(TransportError::Io(std::io::Error::last_os_error()));
        }
        Ok(sdn.device_number)
    }

    fn guid_to_bytes(guid: &windows_sys::core::GUID) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..4].copy_from_slice(&guid.data1.to_le_bytes());
        out[4..6].copy_from_slice(&guid.data2.to_le_bytes());
        out[6..8].copy_from_slice(&guid.data3.to_le_bytes());
        out[8..16].copy_from_slice(&guid.data4);
        out
    }

    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
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
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::path::{Path, PathBuf};

    use crate::{InquiryData, ScsiResponse, ScsiTransport, TransportError};

    #[derive(Debug, Clone)]
    pub struct SetupApiUsbMscCandidate {
        pub device_path: PathBuf,
        pub vid: Option<u16>,
        pub pid: Option<u16>,
        pub interface_guid: [u8; 16],
        pub note: String,
        pub visible_selector: Option<String>,
    }

    pub const DEFAULT_USB_VID_ALLOWLIST: &[u16] = &[0x4A4C];

    pub fn enumerate_usb_msc_candidates_with_allowlist(
        _allowlist: &[u16],
    ) -> Result<Vec<SetupApiUsbMscCandidate>, TransportError> {
        Err(TransportError::Unsupported)
    }

    pub fn enumerate_usb_msc_candidates() -> Result<Vec<SetupApiUsbMscCandidate>, TransportError> {
        enumerate_usb_msc_candidates_with_allowlist(DEFAULT_USB_VID_ALLOWLIST)
    }

    pub fn candidate_windows_paths() -> Result<Vec<PathBuf>, TransportError> {
        let candidates = enumerate_usb_msc_candidates()?;
        Ok(candidates.into_iter().map(|c| c.device_path).collect())
    }

    pub fn resolve_visible_selector_to_candidate(
        selector: &str,
    ) -> Result<SetupApiUsbMscCandidate, TransportError> {
        let _ = selector;
        Err(TransportError::Unsupported)
    }

    pub fn resolve_visible_selector_to_path(selector: &str) -> Result<PathBuf, TransportError> {
        let _ = selector;
        Err(TransportError::Unsupported)
    }

    pub struct WindowsScsiDevice {
        path: PathBuf,
        timeout_ms: u32,
    }

    impl WindowsScsiDevice {
        pub fn open(path: impl AsRef<Path>) -> Result<Self, TransportError> {
            Self::open_with_timeout(path, 5000)
        }

        pub fn open_with_timeout(
            path: impl AsRef<Path>,
            timeout_ms: u32,
        ) -> Result<Self, TransportError> {
            let path = path.as_ref().to_path_buf();
            if !path.exists() {
                return Err(TransportError::DeviceNotFound(path.display().to_string()));
            }
            Ok(Self { path, timeout_ms })
        }

        pub fn timeout_ms(&self) -> u32 {
            self.timeout_ms
        }
    }

    impl ScsiTransport for WindowsScsiDevice {
        fn path(&self) -> &Path {
            &self.path
        }

        fn exec(
            &mut self,
            _cdb: &[u8],
            _data_out: Option<&[u8]>,
            _data_in_len: usize,
        ) -> Result<ScsiResponse, TransportError> {
            Err(TransportError::Unsupported)
        }

        fn inquiry(&mut self) -> Result<InquiryData, TransportError> {
            Err(TransportError::Unsupported)
        }
    }
}

pub use imp::{
    DEFAULT_USB_VID_ALLOWLIST, SetupApiUsbMscCandidate, WindowsScsiDevice, candidate_windows_paths,
    enumerate_usb_msc_candidates, enumerate_usb_msc_candidates_with_allowlist,
    resolve_visible_selector_to_candidate, resolve_visible_selector_to_path,
};

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod common {
    pub fn looks_like_usb_msc_instance(instance_id: &str) -> bool {
        let upper = instance_id.to_ascii_uppercase();
        upper.starts_with("USBSTOR\\") || upper.starts_with("USB\\")
    }

    pub fn parse_vid_pid(instance_id: &str) -> (Option<u16>, Option<u16>) {
        let upper = instance_id.to_ascii_uppercase();
        let vid = extract_hex_field(&upper, "VID_");
        let pid = extract_hex_field(&upper, "PID_");
        (vid, pid)
    }

    pub fn matches_allowlist(vid: Option<u16>, allowlist: &[u16]) -> bool {
        let Some(vid) = vid else {
            return false;
        };
        allowlist.contains(&vid)
    }

    fn extract_hex_field(s: &str, key: &str) -> Option<u16> {
        let pos = s.find(key)?;
        let hex = s.get(pos + key.len()..pos + key.len() + 4)?;
        u16::from_str_radix(hex, 16).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::common::{looks_like_usb_msc_instance, matches_allowlist, parse_vid_pid};

    #[test]
    fn usb_storage_instance_is_recognized() {
        assert!(looks_like_usb_msc_instance(
            r"USBSTOR\DISK&VEN_JL&PROD_UBOOT&REV_1.00\123456"
        ));
        assert!(looks_like_usb_msc_instance(r"USB\VID_4A4C&PID_1234\ABCDEF"));
        assert!(!looks_like_usb_msc_instance(r"PCI\VEN_8086&DEV_1234\0"));
    }

    #[test]
    fn vid_pid_are_parsed_case_insensitively() {
        assert_eq!(
            parse_vid_pid(r"USB\VID_4a4c&PID_00f1\ABCDEF"),
            (Some(0x4A4C), Some(0x00F1))
        );
        assert_eq!(
            parse_vid_pid(r"USBSTOR\DISK&VEN_JL&PROD_X\NO_VID_PID"),
            (None, None)
        );
    }

    #[test]
    fn allowlist_requires_known_vid() {
        let allow = [0x4A4C, 0x1234];
        assert!(matches_allowlist(Some(0x4A4C), &allow));
        assert!(!matches_allowlist(Some(0x9999), &allow));
        assert!(!matches_allowlist(None, &allow));
    }
}
