#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    Uboot1,
    LoaderV1,
    LoaderV2,
}

#[derive(Debug, Clone)]
pub struct ChipFamily {
    pub family: &'static str,
    pub names: &'static [&'static str],
    pub notes: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct LoaderInfo {
    pub family: &'static str,
    pub file: &'static str,
    pub address: u32,
    pub protocol: ProtocolVersion,
    pub argument: Option<u32>,
    pub block_size: Option<usize>,
    pub encryption: Option<&'static str>,
}

pub const CHIP_FAMILIES: &[ChipFamily] = &[
    ChipFamily {
        family: "br17",
        names: &["AC690N"],
        notes: None,
    },
    ChipFamily {
        family: "br23",
        names: &["AC695N", "AC635N"],
        notes: None,
    },
    ChipFamily {
        family: "br28",
        names: &["AC701N"],
        notes: Some("current SDK family"),
    },
];

pub const USB_LOADERS: &[LoaderInfo] = &[LoaderInfo {
    family: "br17",
    file: "loaderblobs/usb/br17loader.bin",
    address: 0x2000,
    protocol: ProtocolVersion::LoaderV2,
    argument: Some(0x0011),
    block_size: None,
    encryption: None,
}];
