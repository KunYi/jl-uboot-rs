use jl_msc::{build_cdb, validate_response_command};
use jl_sg::{InquiryData, ScsiTransport, TransportError};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineDeviceType {
    None = 0x00,
    Sdram = 0x01,
    SdCard = 0x02,
    Spi0Nor = 0x03,
    Spi0Nand = 0x04,
    Otp = 0x05,
    Spi1Nor = 0x16,
    Spi1Nand = 0x17,
    SpiFlash = 0x101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    Uboot1,
    LoaderV1,
    LoaderV2,
}

#[derive(Debug, Clone)]
pub struct OnlineDeviceInfo {
    pub kind: OnlineDeviceType,
    pub id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ProbeInfo {
    pub inquiry: InquiryData,
    pub online_device: OnlineDeviceInfo,
    pub flash_id: u32,
    pub usb_buffer_size: u32,
    pub loader_version: Option<String>,
    pub maskrom_id: u32,
}

#[derive(Debug, Clone)]
pub struct ChipKeyRaw {
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderV1FlashSelect {
    Code = 0,
    Data = 1,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Msc(#[from] jl_msc::MscError),
    #[error("unsupported protocol path")]
    UnsupportedProtocol,
    #[error("response too short")]
    ResponseTooShort,
    #[error("response payload too short: expected at least {expected}, got {actual}")]
    ResponsePayloadTooShort { expected: usize, actual: usize },
    #[error("unexpected online device type: 0x{0:02x}")]
    UnexpectedOnlineDevice(u8),
    #[error("length too large for protocol field: {0}")]
    LengthTooLarge(usize),
    #[error("invalid loader version string")]
    InvalidLoaderVersion,
}

pub trait FlashAccess {
    fn flash_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, Error>;
    fn flash_write(&mut self, addr: u32, data: &[u8]) -> Result<(), Error>;
    fn flash_erase_sector(&mut self, addr: u32) -> Result<(), Error>;
    fn flash_erase_block(&mut self, addr: u32) -> Result<(), Error>;
    fn flash_erase_chip(&mut self) -> Result<(), Error>;
}

pub trait MemAccess {
    fn mem_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, Error>;
    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), Error>;
    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), Error>;
}

pub struct JlDevice<T: ScsiTransport> {
    transport: T,
    protocol: ProtocolKind,
}

impl<T: ScsiTransport> JlDevice<T> {
    pub fn new(transport: T, protocol: ProtocolKind) -> Self {
        Self {
            transport,
            protocol,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn inquiry(&mut self) -> Result<InquiryData, Error> {
        self.transport.inquiry().map_err(Error::from)
    }

    pub fn online_device(&mut self) -> Result<OnlineDeviceInfo, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC0A, &[])?;
                if payload.is_empty() {
                    return Err(Error::ResponseTooShort);
                }
                let kind = match payload[0] {
                    0x00 => OnlineDeviceType::None,
                    0x01 => OnlineDeviceType::Sdram,
                    0x02 => OnlineDeviceType::SdCard,
                    0x03 => OnlineDeviceType::Spi0Nor,
                    0x04 => OnlineDeviceType::Spi0Nand,
                    0x05 => OnlineDeviceType::Otp,
                    0x16 => OnlineDeviceType::Spi1Nor,
                    0x17 => OnlineDeviceType::Spi1Nand,
                    other => return Err(Error::UnexpectedOnlineDevice(other)),
                };
                let id = if payload.len() >= 6 {
                    Some(u32::from_le_bytes([
                        payload[2], payload[3], payload[4], payload[5],
                    ]))
                } else {
                    None
                };
                Ok(OnlineDeviceInfo { kind, id })
            }
            ProtocolKind::LoaderV1 => {
                let payload = self.cmd_exec(0xFC0B, &[])?;
                if payload.is_empty() {
                    return Err(Error::ResponseTooShort);
                }
                let kind = match payload[0] {
                    0x00 => OnlineDeviceType::None,
                    0x01 => OnlineDeviceType::SpiFlash,
                    0x02 => OnlineDeviceType::SdCard,
                    other => return Err(Error::UnexpectedOnlineDevice(other)),
                };
                Ok(OnlineDeviceInfo { kind, id: None })
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn read_id(&mut self) -> Result<u32, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC0B, &[])?;
                ensure_payload_len(&payload, 3)?;
                Ok(u32::from_be_bytes([0, payload[0], payload[1], payload[2]]))
            }
            ProtocolKind::LoaderV1 => {
                let payload = self.cmd_exec(0xFC00, &[])?;
                ensure_payload_len(&payload, 3)?;
                Ok(u32::from_be_bytes([0, payload[0], payload[1], payload[2]]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn usb_buffer_size(&mut self) -> Result<u32, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC14, &[])?;
                ensure_payload_len(&payload, 4)?;
                Ok(u32::from_be_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                ]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn loader_version(&mut self) -> Result<Option<String>, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let cdb = build_cdb(0xFC15, &[])?;
                let response = self.transport.exec(&cdb, None, 16)?;
                if response.data.len() < 6 {
                    return Ok(None);
                }
                let raw = [
                    response.data[5],
                    response.data[4],
                    response.data[3],
                    response.data[2],
                ];
                let text = std::str::from_utf8(&raw).map_err(|_| Error::InvalidLoaderVersion)?;
                Ok(Some(text.trim_matches(char::from(0)).to_string()))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn maskrom_id(&mut self) -> Result<u32, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC16, &[])?;
                ensure_payload_len(&payload, 4)?;
                Ok(u32::from_be_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                ]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn run_app(&mut self, arg: u32) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                self.cmd_exec(0xFC0C, &arg.to_be_bytes())?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn read_status(&mut self) -> Result<u8, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC03, &[])?;
                ensure_payload_len(&payload, 1)?;
                Ok(payload[0])
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn set_flash_cmds(&mut self, cmds: &[u8]) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 =
                    u16::try_from(cmds.len()).map_err(|_| Error::LengthTooLarge(cmds.len()))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&[0, 0, 0, 0]);
                args.extend_from_slice(&len16.to_be_bytes());
                let _ = self.cmd_exec_datain(0xFC0D, &args, cmds.len())?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn flash_crc16(&mut self, addr: u32, len: usize) -> Result<u16, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                let payload = self.cmd_exec(0xFC0E, &args)?;
                ensure_payload_len(&payload, 2)?;
                Ok(u16::from_be_bytes([payload[0], payload[1]]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn flash_crc16_raw(&mut self, addr: u32, len: usize) -> Result<u16, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                let payload = self.cmd_exec(0xFC13, &args)?;
                ensure_payload_len(&payload, 2)?;
                Ok(u16::from_be_bytes([payload[0], payload[1]]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn probe_info(&mut self) -> Result<ProbeInfo, Error> {
        let inquiry = self.inquiry()?;
        let online_device = self.online_device()?;
        let flash_id = self.read_id()?;
        let usb_buffer_size = self.usb_buffer_size()?;
        let loader_version = self.loader_version()?;
        let maskrom_id = self.maskrom_id()?;
        Ok(ProbeInfo {
            inquiry,
            online_device,
            flash_id,
            usb_buffer_size,
            loader_version,
            maskrom_id,
        })
    }

    pub fn read_key_raw(&mut self, arg: u32) -> Result<ChipKeyRaw, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC09, &arg.to_be_bytes())?;
                Ok(ChipKeyRaw { payload })
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn chip_key(&mut self, arg: u32) -> Result<u16, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let payload = self.cmd_exec(0xFC09, &arg.to_be_bytes())?;
                ensure_payload_len(&payload, 6)?;
                let src = [payload[5], payload[4]];
                let decoded = jl_crc_cipher_bytes(&src, 0xffff_ffff);
                Ok(u16::from_le_bytes([decoded[0], decoded[1]]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn write_chipkey(&mut self, key: u32, vpp: u32) -> Result<u32, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let mut args = Vec::with_capacity(9);
                args.extend_from_slice(&key.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&vpp.to_be_bytes());
                let payload = self.cmd_exec(0xFC12, &args)?;
                ensure_payload_len(&payload, 4)?;
                Ok(u32::from_be_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                ]))
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn flash_select(&mut self, sel: LoaderV1FlashSelect) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV1 => {
                self.cmd_exec(0xFC0C, &[sel as u8])?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    pub fn mem_write_rxgp(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::Uboot1 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let crc = jl_crc16(data);
                let mut args = Vec::with_capacity(8);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&crc.to_le_bytes());
                self.cmd_exec_dataout(0xFB31, &args, data)
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn cmd_exec(&mut self, cmd: u16, args: &[u8]) -> Result<Vec<u8>, Error> {
        let cdb = build_cdb(cmd, args)?;
        let response = self.transport.exec(&cdb, None, 16)?;
        validate_response_command(cmd, &response.data)?;
        Ok(response.data[2..].to_vec())
    }

    fn cmd_exec_datain(&mut self, cmd: u16, args: &[u8], len: usize) -> Result<Vec<u8>, Error> {
        let cdb = build_cdb(cmd, args)?;
        let response = self.transport.exec(&cdb, None, len)?;
        Ok(response.data)
    }

    fn cmd_exec_dataout(&mut self, cmd: u16, args: &[u8], data: &[u8]) -> Result<(), Error> {
        let cdb = build_cdb(cmd, args)?;
        let _ = self.transport.exec(&cdb, Some(data), 0)?;
        Ok(())
    }
}

impl<T: ScsiTransport> FlashAccess for JlDevice<T> {
    fn flash_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                self.cmd_exec_datain(0xFD05, &args, len)
            }
            ProtocolKind::LoaderV1 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                self.cmd_exec_datain(0xFD01, &args, len)
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn flash_write(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let crc = jl_crc16(data);
                let mut args = Vec::with_capacity(8);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&crc.to_le_bytes());
                self.cmd_exec_dataout(0xFB04, &args, data)
            }
            ProtocolKind::LoaderV1 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let crc = jl_crc16(data);
                let mut args = Vec::with_capacity(8);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&crc.to_le_bytes());
                self.cmd_exec_dataout(0xFB01, &args, data)
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn flash_erase_sector(&mut self, addr: u32) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                self.cmd_exec(0xFB01, &addr.to_be_bytes())?;
                Ok(())
            }
            ProtocolKind::LoaderV1 => {
                self.cmd_exec(0xFB03, &addr.to_be_bytes())?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn flash_erase_block(&mut self, addr: u32) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                self.cmd_exec(0xFB00, &addr.to_be_bytes())?;
                Ok(())
            }
            ProtocolKind::LoaderV1 => {
                self.cmd_exec(0xFB00, &addr.to_be_bytes())?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn flash_erase_chip(&mut self) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                self.cmd_exec(0xFB02, &[])?;
                Ok(())
            }
            ProtocolKind::LoaderV1 => {
                self.cmd_exec(0xFB02, &[])?;
                Ok(())
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }
}

impl<T: ScsiTransport> MemAccess for JlDevice<T> {
    fn mem_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                self.cmd_exec_datain(0xFD07, &args, len)
            }
            ProtocolKind::Uboot1 => {
                let len16 = u16::try_from(len).map_err(|_| Error::LengthTooLarge(len))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                self.cmd_exec_datain(0xFD07, &args, len)
            }
            _ => Err(Error::UnsupportedProtocol),
        }
    }

    fn mem_write(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let crc = jl_crc16(data);
                let mut args = Vec::with_capacity(8);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&crc.to_le_bytes());
                self.cmd_exec_dataout(0xFB06, &args, data)
            }
            ProtocolKind::LoaderV1 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                self.cmd_exec_dataout(0xFB04, &args, data)
            }
            ProtocolKind::Uboot1 => {
                let len16 =
                    u16::try_from(data.len()).map_err(|_| Error::LengthTooLarge(data.len()))?;
                let crc = jl_crc16(data);
                let mut args = Vec::with_capacity(8);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&len16.to_be_bytes());
                args.push(0);
                args.extend_from_slice(&crc.to_le_bytes());
                self.cmd_exec_dataout(0xFB06, &args, data)
            }
        }
    }

    fn mem_jump(&mut self, addr: u32, arg: u32) -> Result<(), Error> {
        match self.protocol {
            ProtocolKind::LoaderV2 => {
                let arg16 = u16::try_from(arg).map_err(|_| Error::LengthTooLarge(arg as usize))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&arg16.to_be_bytes());
                self.cmd_exec(0xFB08, &args)?;
                Ok(())
            }
            ProtocolKind::LoaderV1 => {
                let arg16 = u16::try_from(arg).map_err(|_| Error::LengthTooLarge(arg as usize))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&arg16.to_be_bytes());
                self.cmd_exec(0xFB09, &args)?;
                Ok(())
            }
            ProtocolKind::Uboot1 => {
                let arg16 = u16::try_from(arg).map_err(|_| Error::LengthTooLarge(arg as usize))?;
                let mut args = Vec::with_capacity(6);
                args.extend_from_slice(&addr.to_be_bytes());
                args.extend_from_slice(&arg16.to_be_bytes());
                self.cmd_exec(0xFB08, &args)?;
                Ok(())
            }
        }
    }
}

fn ensure_payload_len(payload: &[u8], expected: usize) -> Result<(), Error> {
    if payload.len() < expected {
        return Err(Error::ResponsePayloadTooShort {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn jl_crc16(data: &[u8]) -> u16 {
    jl_crc16_seed(data, 0)
}

fn jl_crc16_seed(data: &[u8], mut crc: u16) -> u16 {
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn jl_crc_cipher_bytes(data: &[u8], key: u32) -> Vec<u8> {
    const MAGIC: &[u8] = &[
        0xC3, 0xCF, 0xC0, 0xE8, 0xCE, 0xD2, 0xB0, 0xAE, 0xC4, 0xE3, 0xA3, 0xAC, 0xD3, 0xF1, 0xC1,
        0xD6,
    ];

    let mut out = data.to_vec();
    let mut crc = jl_crc16_seed(&(key >> 16).to_le_bytes()[..2], (key & 0xffff) as u16);

    for (i, b) in out.iter_mut().enumerate() {
        crc = jl_crc16_seed(&[MAGIC[i % MAGIC.len()]], crc);
        *b ^= (crc & 0xff) as u8;
    }

    out
}

#[allow(dead_code)]
fn _legacy_jl_crc16_reference(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use jl_msc::build_cdb;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    struct ExpectedExec {
        cdb: [u8; 16],
        data_out: Option<Vec<u8>>,
        data_in_len: usize,
        response: Result<Vec<u8>, TransportError>,
    }

    #[derive(Debug)]
    struct MockTransport {
        path: PathBuf,
        inquiry: InquiryData,
        execs: VecDeque<ExpectedExec>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                path: PathBuf::from("/dev/mocksg0"),
                inquiry: InquiryData {
                    vendor: "JIELI".to_string(),
                    product: "UBOOT".to_string(),
                    revision: "1.00".to_string(),
                    raw: vec![0; 36],
                },
                execs: VecDeque::new(),
            }
        }

        fn expect_query(mut self, cmd: u16, args: &[u8], payload: &[u8]) -> Self {
            let mut response = Vec::with_capacity(payload.len() + 2);
            response.extend_from_slice(&cmd.to_be_bytes());
            response.extend_from_slice(payload);
            self.execs.push_back(ExpectedExec {
                cdb: build_cdb(cmd, args).expect("query cdb"),
                data_out: None,
                data_in_len: 16,
                response: Ok(response),
            });
            self
        }

        fn expect_datain(mut self, cmd: u16, args: &[u8], len: usize, response: &[u8]) -> Self {
            self.execs.push_back(ExpectedExec {
                cdb: build_cdb(cmd, args).expect("datain cdb"),
                data_out: None,
                data_in_len: len,
                response: Ok(response.to_vec()),
            });
            self
        }

        fn expect_dataout(mut self, cmd: u16, args: &[u8], data: &[u8]) -> Self {
            self.execs.push_back(ExpectedExec {
                cdb: build_cdb(cmd, args).expect("dataout cdb"),
                data_out: Some(data.to_vec()),
                data_in_len: 0,
                response: Ok(Vec::new()),
            });
            self
        }
    }

    impl ScsiTransport for MockTransport {
        fn path(&self) -> &Path {
            &self.path
        }

        fn exec(
            &mut self,
            cdb: &[u8],
            data_out: Option<&[u8]>,
            data_in_len: usize,
        ) -> Result<jl_sg::ScsiResponse, TransportError> {
            let expected = self.execs.pop_front().expect("unexpected exec call");
            assert_eq!(cdb, expected.cdb);
            assert_eq!(data_in_len, expected.data_in_len);
            assert_eq!(data_out, expected.data_out.as_deref());
            expected
                .response
                .map(|data| jl_sg::ScsiResponse { data, resid: 0 })
        }

        fn inquiry(&mut self) -> Result<InquiryData, TransportError> {
            Ok(self.inquiry.clone())
        }
    }

    #[test]
    fn crc16_matches_legacy_reference() {
        let data = b"JieLi test payload";
        assert_eq!(jl_crc16(data), _legacy_jl_crc16_reference(data));
    }

    #[test]
    fn crc16_seed_is_consistent() {
        let a = jl_crc16_seed(b"hello", 0x1234);
        let b = jl_crc16_seed(b"hello", 0x1234);
        assert_eq!(a, b);
    }

    #[test]
    fn chipkey_cipher_is_reversible_with_same_key() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let enc = jl_crc_cipher_bytes(&data, 0xffff_ffff);
        let dec = jl_crc_cipher_bytes(&enc, 0xffff_ffff);
        assert_eq!(dec, data);
    }

    #[test]
    fn ensure_payload_len_reports_actual_length() {
        let err = ensure_payload_len(&[1, 2], 4).expect_err("must fail");
        match err {
            Error::ResponsePayloadTooShort { expected, actual } => {
                assert_eq!(expected, 4);
                assert_eq!(actual, 2);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn online_device_type_numeric_values_are_stable() {
        assert_eq!(OnlineDeviceType::Spi0Nor as u32, 0x03);
        assert_eq!(OnlineDeviceType::Spi1Nor as u32, 0x16);
        assert_eq!(OnlineDeviceType::SpiFlash as u32, 0x101);
    }

    #[test]
    fn probe_info_collects_loaderv2_fields() {
        let transport = MockTransport::new()
            .expect_query(0xFC0A, &[], &[0x03, 0x00, 0x78, 0x56, 0x34, 0x12])
            .expect_query(0xFC0B, &[], &[0x12, 0x34, 0x56])
            .expect_query(0xFC14, &[], &[0x00, 0x00, 0x10, 0x00])
            .expect_datain(0xFC15, &[], 16, &[0, 0, b'1', b'2', b'3', b'4'])
            .expect_query(0xFC16, &[], &[0xAA, 0xBB, 0xCC, 0xDD]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let info = dev.probe_info().expect("probe info");
        assert_eq!(info.inquiry.vendor, "JIELI");
        assert_eq!(info.online_device.kind, OnlineDeviceType::Spi0Nor);
        assert_eq!(info.online_device.id, Some(0x1234_5678));
        assert_eq!(info.flash_id, 0x12_34_56);
        assert_eq!(info.usb_buffer_size, 0x1000);
        assert_eq!(info.loader_version.as_deref(), Some("4321"));
        assert_eq!(info.maskrom_id, 0xAABB_CCDD);
    }

    #[test]
    fn loader_v1_flash_read_uses_expected_command() {
        let transport = MockTransport::new().expect_datain(
            0xFD01,
            &[0x00, 0x01, 0x00, 0x00, 0x00, 0x04],
            4,
            &[0xDE, 0xAD, 0xBE, 0xEF],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV1);
        let data = dev.flash_read(0x0001_0000, 4).expect("flash read");
        assert_eq!(data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn mem_write_rxgp_uses_crc_and_command() {
        let payload = [0x10, 0x20, 0x30, 0x40];
        let crc = jl_crc16(&payload);
        let mut args = Vec::new();
        args.extend_from_slice(&0x1F00_0000u32.to_be_bytes());
        args.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        args.push(0);
        args.extend_from_slice(&crc.to_le_bytes());
        let transport = MockTransport::new().expect_dataout(0xFB31, &args, &payload);
        let mut dev = JlDevice::new(transport, ProtocolKind::Uboot1);

        dev.mem_write_rxgp(0x1F00_0000, &payload)
            .expect("rxgp write");
    }

    #[test]
    fn flash_crc16_uses_expected_command_and_decodes_result() {
        let transport = MockTransport::new().expect_query(
            0xFC0E,
            &[0x00, 0x00, 0x10, 0x00, 0x00, 0x80],
            &[0x12, 0x34],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let crc = dev.flash_crc16(0x0000_1000, 0x80).expect("flash crc16");
        assert_eq!(crc, 0x1234);
    }

    #[test]
    fn flash_crc16_raw_uses_expected_command_and_decodes_result() {
        let transport = MockTransport::new().expect_query(
            0xFC13,
            &[0x00, 0x00, 0x20, 0x00, 0x00, 0x40],
            &[0x56, 0x78],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let crc = dev
            .flash_crc16_raw(0x0000_2000, 0x40)
            .expect("flash crc16 raw");
        assert_eq!(crc, 0x5678);
    }

    #[test]
    fn read_status_uses_expected_command_and_decodes_result() {
        let transport = MockTransport::new().expect_query(0xFC03, &[], &[0xA5]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let status = dev.read_status().expect("read status");
        assert_eq!(status, 0xA5);
    }

    #[test]
    fn maskrom_id_uses_expected_command_and_decodes_result() {
        let transport = MockTransport::new().expect_query(0xFC16, &[], &[0x11, 0x22, 0x33, 0x44]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let id = dev.maskrom_id().expect("maskrom id");
        assert_eq!(id, 0x1122_3344);
    }

    #[test]
    fn read_key_raw_preserves_payload_bytes() {
        let transport = MockTransport::new().expect_query(
            0xFC09,
            &[0x00, 0xAC, 0x69, 0x00],
            &[0x10, 0x20, 0x30, 0x40, 0x50],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let raw = dev.read_key_raw(0x00AC6900).expect("read key raw");
        assert_eq!(raw.payload, vec![0x10, 0x20, 0x30, 0x40, 0x50]);
    }

    #[test]
    fn chip_key_decodes_expected_value() {
        let encoded = jl_crc_cipher_bytes(&[0x34, 0x12], 0xffff_ffff);
        let transport = MockTransport::new().expect_query(
            0xFC09,
            &[0x00, 0xAC, 0x69, 0x00],
            &[0x00, 0x00, 0x00, 0x00, encoded[1], encoded[0]],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let key = dev.chip_key(0x00AC6900).expect("chip key");
        assert_eq!(key, 0x1234);
    }

    #[test]
    fn chip_key_requires_minimum_payload_length() {
        let transport = MockTransport::new().expect_query(
            0xFC09,
            &[0x00, 0xAC, 0x69, 0x00],
            &[0x00, 0x01, 0x02, 0x03, 0x04],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let err = dev.chip_key(0x00AC6900).expect_err("must fail");
        match err {
            Error::ResponsePayloadTooShort { expected, actual } => {
                assert_eq!(expected, 6);
                assert_eq!(actual, 5);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn set_flash_cmds_uses_datain_path_with_expected_length() {
        let cmds = [0x03, 0x0B, 0x05, 0xD8, 0x20, 0x52, 0x60, 0xC7];
        let transport = MockTransport::new().expect_datain(
            0xFC0D,
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x08],
            cmds.len(),
            &cmds,
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        dev.set_flash_cmds(&cmds).expect("set flash cmds");
    }

    #[test]
    fn run_app_uses_expected_command() {
        let transport = MockTransport::new().expect_query(0xFC0C, &[0x00, 0x00, 0x00, 0x02], &[]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        dev.run_app(2).expect("run app");
    }

    #[test]
    fn write_chipkey_uses_expected_payload() {
        let transport = MockTransport::new().expect_query(
            0xFC12,
            &[0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x13, 0x88],
            &[0xAA, 0xBB, 0xCC, 0xDD],
        );
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let value = dev.write_chipkey(0x1234_5678, 5000).expect("write chipkey");
        assert_eq!(value, 0xAABB_CCDD);
    }

    #[test]
    fn loader_v1_flash_select_uses_expected_command() {
        let transport = MockTransport::new().expect_query(0xFC0C, &[0x01], &[]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV1);

        dev.flash_select(LoaderV1FlashSelect::Data)
            .expect("flash select");
    }

    #[test]
    fn mem_jump_uses_protocol_specific_command() {
        let transport =
            MockTransport::new().expect_query(0xFB09, &[0x1F, 0x00, 0x00, 0x00, 0x00, 0x02], &[]);
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV1);

        dev.mem_jump(0x1F00_0000, 2).expect("mem jump");
    }

    #[test]
    fn loader_v1_usb_buffer_size_is_unsupported() {
        let transport = MockTransport::new();
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV1);

        let err = dev.usb_buffer_size().expect_err("must fail");
        assert!(matches!(err, Error::UnsupportedProtocol));
    }

    #[test]
    fn uboot1_flash_read_is_unsupported() {
        let transport = MockTransport::new();
        let mut dev = JlDevice::new(transport, ProtocolKind::Uboot1);

        let err = dev.flash_read(0, 4).expect_err("must fail");
        assert!(matches!(err, Error::UnsupportedProtocol));
    }

    #[test]
    fn command_mismatch_surfaces_as_msc_error() {
        let mut transport = MockTransport::new();
        transport.execs.push_back(ExpectedExec {
            cdb: build_cdb(0xFC0B, &[]).expect("cdb"),
            data_out: None,
            data_in_len: 16,
            response: Ok(vec![0xFC, 0x0A, 0x03]),
        });
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let err = dev.read_id().expect_err("must fail");
        match err {
            Error::Msc(jl_msc::MscError::CommandMismatch { expected, actual }) => {
                assert_eq!(expected, 0xFC0B);
                assert_eq!(actual, 0xFC0A);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn transport_error_is_preserved() {
        let mut transport = MockTransport::new();
        transport.execs.push_back(ExpectedExec {
            cdb: build_cdb(0xFC16, &[]).expect("cdb"),
            data_out: None,
            data_in_len: 16,
            response: Err(TransportError::SgIo("timeout".to_string())),
        });
        let mut dev = JlDevice::new(transport, ProtocolKind::LoaderV2);

        let err = dev.maskrom_id().expect_err("must fail");
        match err {
            Error::Transport(TransportError::SgIo(text)) => assert_eq!(text, "timeout"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
