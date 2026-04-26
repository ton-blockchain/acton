use tycho_types::cell::{Cell, CellBuilder, CellSlice};

/// Build a snake-bytes cell chain.
#[must_use]
pub fn build_snake_bytes_cell(bytes: &[u8]) -> Cell {
    let total_bits = bytes.len() * 8;

    // Keep some free space in each cell for potential prefixes.
    if total_bits <= 1015 {
        let mut builder = CellBuilder::new();
        builder.store_raw(bytes, total_bits as u16).ok();
        return builder.build().expect("cannot build cell");
    }

    let mut remaining_bytes = bytes;
    let mut cell_data = Vec::new();

    while !remaining_bytes.is_empty() {
        let chunk_size = std::cmp::min(remaining_bytes.len(), 126);
        let chunk = &remaining_bytes[..chunk_size];
        cell_data.push((chunk, chunk.len() * 8));
        remaining_bytes = &remaining_bytes[chunk_size..];
    }

    let mut next_cell: Option<Cell> = None;
    for (chunk, bits) in cell_data.into_iter().rev() {
        let mut builder = CellBuilder::new();
        builder.store_raw(chunk, bits as u16).ok();

        if let Some(next) = next_cell {
            builder.store_reference(next).ok();
        }

        next_cell = Some(builder.build().expect("cannot build cell"));
    }

    next_cell.expect("snake string must have at least one cell")
}

/// Parse snake bytes from a cell.
#[must_use]
pub fn parse_snake_bytes(cell: &Cell) -> Option<Vec<u8>> {
    let mut parser = cell.as_slice_allow_exotic();
    parse_snake_bytes_slice(&mut parser)
}

/// Parse a snake string from a cell.
#[must_use]
pub fn parse_snake_string(cell: &Cell) -> Option<String> {
    String::from_utf8(parse_snake_bytes(cell)?).ok()
}

/// Parse snake bytes from a cell slice.
#[must_use]
pub fn parse_snake_bytes_slice(parser: &mut CellSlice<'_>) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let bits_to_load = parser.size_bits();
    if !bits_to_load.is_multiple_of(8) {
        return None;
    }

    let mut chunk = vec![0u8; bits_to_load.div_ceil(8) as usize];
    parser.load_raw(&mut chunk, bits_to_load).ok()?;
    bytes.extend_from_slice(&chunk);

    if parser.size_refs() == 0 {
        return Some(bytes);
    }

    let mut next_data_ref = parser.load_reference_cloned().ok()?;
    loop {
        let mut parser = next_data_ref.as_slice_allow_exotic();
        let bits_to_load = parser.size_bits();
        if !bits_to_load.is_multiple_of(8) {
            return None;
        }

        let mut chunk = vec![0u8; bits_to_load.div_ceil(8) as usize];
        parser.load_raw(&mut chunk, bits_to_load).ok()?;
        bytes.extend_from_slice(&chunk);

        if parser.size_refs() == 0 {
            break;
        }

        next_data_ref = match parser.load_reference_cloned() {
            Ok(cell) => cell,
            Err(_) => break,
        };
    }

    Some(bytes)
}

/// Parse a snake string from a cell slice.
#[must_use]
pub fn parse_snake_string_slice(parser: &mut CellSlice<'_>) -> Option<String> {
    String::from_utf8(parse_snake_bytes_slice(parser)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_string_roundtrip_small_and_large() {
        for input in ["Hello World".to_owned(), "A".repeat(200)] {
            let cell = build_snake_bytes_cell(input.as_bytes());
            assert_eq!(parse_snake_string(&cell), Some(input));
        }
    }

    #[test]
    fn snake_string_roundtrip_empty() {
        let cell = build_snake_bytes_cell(b"");
        assert_eq!(parse_snake_string(&cell), Some(String::new()));
    }
}
