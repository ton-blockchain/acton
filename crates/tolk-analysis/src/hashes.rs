use crc::{CRC_16_XMODEM, CRC_32_ISO_HDLC, Crc};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub(crate) const fn crc16(value: &[u8]) -> u16 {
    CRC16.checksum(value)
}

pub(crate) const fn crc32(value: &[u8]) -> u32 {
    CRC32.checksum(value)
}

#[must_use]
pub fn compute_get_method_id(name: &str) -> u32 {
    u32::from(crc16(name.as_bytes())) | 0x1_0000
}
