use std::sync::{Arc, LazyLock};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily};
use tycho_types::dict::Dict;

pub const DEFAULT_CONFIG: &str = include_str!("default_config.boc64");

pub static DEFAULT_CONFIG_CELL: LazyLock<Cell> = LazyLock::new(|| {
    Boc::decode_base64(DEFAULT_CONFIG).expect("constant config must be valid BoC")
});

pub static DEFAULT_CONFIG_DICT: LazyLock<Arc<Dict<u32, Cell>>> = LazyLock::new(|| {
    let mut slice = DEFAULT_CONFIG_CELL.as_slice_allow_exotic();
    Arc::new(
        Dict::load_from_root_ext(&mut slice, Cell::empty_context())
            .expect("constant config must be valid Dict"),
    )
});
