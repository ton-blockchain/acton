use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

/// Compatibility helpers to keep legacy ArcCell-like call sites compiling
/// while tuple cells are backed by `tycho_types::cell::Cell`.
pub trait CellCompatExt {
    fn from_boc(data: &[u8]) -> anyhow::Result<Cell>;
    fn from_boc_b64(data: &str) -> anyhow::Result<Cell>;
    fn from_boc_hex(data: &str) -> anyhow::Result<Cell>;

    fn to_boc(&self, _has_index: bool) -> anyhow::Result<Vec<u8>>;
    fn to_boc_b64(&self, _has_index: bool) -> anyhow::Result<String>;
    fn to_boc_hex(&self, _has_index: bool) -> anyhow::Result<String>;

    fn to_arc(&self) -> Cell;
}

impl CellCompatExt for Cell {
    fn from_boc(data: &[u8]) -> anyhow::Result<Cell> {
        Boc::decode(data).map_err(Into::into)
    }

    fn from_boc_b64(data: &str) -> anyhow::Result<Cell> {
        Boc::decode_base64(data).map_err(Into::into)
    }

    fn from_boc_hex(data: &str) -> anyhow::Result<Cell> {
        Boc::decode_hex(data).map_err(Into::into)
    }

    fn to_boc(&self, _has_index: bool) -> anyhow::Result<Vec<u8>> {
        Ok(Boc::encode(self))
    }

    fn to_boc_b64(&self, _has_index: bool) -> anyhow::Result<String> {
        Ok(Boc::encode_base64(self))
    }

    fn to_boc_hex(&self, _has_index: bool) -> anyhow::Result<String> {
        Ok(Boc::encode_hex(self))
    }

    fn to_arc(&self) -> Cell {
        self.clone()
    }
}
