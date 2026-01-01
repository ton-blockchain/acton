//! This module contains functionality for working with snake strings.
//! Since TVM doesn't have a separate string format and data is stored in cells
//! of up to 1023 bits (~127 bytes) and up to 4 references to other cells, we have to split strings
//! into chunks and store them as a linked list of cells.
//!
//! To allow for potential prefixes (e.g. 8-bit prefix), we strictly use up to 126 bytes (1008 bits) per cell,
//! leaving at least 15 bits free in each cell.
//!
//! For example, a string of 300 characters will be stored as:
//! ```text
//! cell("first 126 bytes")
//!     cell("second 126 bytes")
//!         cell("remaining 48 bytes")
//! ```
use crate::stack::{Tuple, TupleItem};
use tonlib_core::cell::{ArcCell, CellBuilder};

impl Tuple {
    /// Parse a snake string from a tuple slice.
    ///
    /// If the slice is not a snake string, returns `None`.
    /// This is tricky since we cannot be sure that the slice is a snake string and
    /// not some other data with 8-bit encoding that forms a valid UTF-8 string.
    pub fn parse_snake_string(cell: &ArcCell) -> Option<String> {
        let mut all_bits = Vec::new();

        let mut parser = cell.parser();
        let bits_to_load = cell.bit_len();
        if !bits_to_load.is_multiple_of(8) {
            // this is most likely not a snake string
            return None;
        }

        let bytes_to_load = bits_to_load / 8;

        let bits = parser.load_bits(bytes_to_load * 8).ok()?;
        all_bits.extend_from_slice(&bits);

        if parser.remaining_refs() == 0 {
            // this is a single cell snake string (or the end of one)
            let result = String::from_utf8(all_bits).ok()?;
            return Some(result);
        }

        let mut next_data_ref = parser.next_reference().ok()?;

        loop {
            let mut parser = next_data_ref.parser();

            let bytes_to_load = parser.remaining_bits() / 8;
            let bits = parser.load_bits(bytes_to_load * 8).ok()?;
            all_bits.extend_from_slice(&bits);

            if parser.remaining_refs() == 0 {
                // this cell is the end
                break;
            }

            next_data_ref = parser.next_reference().unwrap()
        }

        let result = String::from_utf8(all_bits).ok()?;
        Some(result)
    }

    /// Push a snake string to the tuple.
    ///
    /// If the string is too long, it will be split into multiple cells automatically.
    pub fn push_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let total_bits = bytes.len() * 8;

        // We leave 8 bits free in each cell for prefixes
        if total_bits <= 1015 {
            // Fast path, the string fits in one cell
            let mut b = CellBuilder::new();
            b.store_bits(total_bits, bytes).unwrap();
            self.push(TupleItem::Slice(b.build().unwrap().into()));
            return;
        }

        let mut remaining_bytes = bytes;
        let mut cell_data = Vec::new();

        while !remaining_bytes.is_empty() {
            let chunk_size = std::cmp::min(remaining_bytes.len(), 126); // 126 bytes = 1008 bits < 1015
            let chunk = &remaining_bytes[..chunk_size];
            cell_data.push((chunk, chunk.len() * 8));
            remaining_bytes = &remaining_bytes[chunk_size..];
        }

        // build cells from last to first
        let mut next_cell: Option<ArcCell> = None;

        for (chunk, bits) in cell_data.into_iter().rev() {
            let mut b = CellBuilder::new();
            b.store_bits(bits, chunk).unwrap();

            if let Some(next) = next_cell {
                b.store_reference(&next).unwrap();
            }

            next_cell = Some(ArcCell::from(b.build().unwrap()));
        }

        let root_cell = next_cell.unwrap();
        self.push(TupleItem::Slice(root_cell));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serde::{parse_tuple, serialize_tuple};
    use crate::stack::Tuple;

    #[test]
    fn test_string_roundtrip() {
        let small_string = "Hello World";
        let mut tuple = Tuple::empty();
        tuple.push_string(small_string);
        let serialized = serialize_tuple(&tuple).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();
        assert_eq!(tuple, deserialized);

        let large_string = "A".repeat(200); // 200 bytes = 1600 bits > 1023
        let mut tuple = Tuple::empty();
        tuple.push_string(&large_string);
        let serialized = serialize_tuple(&tuple).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();
        assert_eq!(tuple, deserialized);
    }

    #[test]
    fn test_empty_string() {
        let empty_string = "";
        let mut tuple = Tuple::empty();
        tuple.push_string(empty_string);
        let serialized = serialize_tuple(&tuple).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();
        assert_eq!(tuple, deserialized);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, Some(empty_string.to_string()));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_boundary_sizes() {
        let test_cases = vec![
            ("a".to_string(), 1),   // 1 byte
            ("a".repeat(126), 126), // 126 bytes (fits in one cell)
            ("a".repeat(127), 127), // 127 bytes (requires two cells)
            ("a".repeat(128), 128), // 128 bytes (requires two cells)
            ("a".repeat(252), 252), // 252 bytes (two full cells: 126 * 2)
            ("a".repeat(253), 253), // 253 bytes (requires three cells)
            ("a".repeat(378), 378), // 378 bytes (three full cells: 126 * 3)
        ];

        for (test_string, expected_len) in test_cases {
            assert_eq!(test_string.len(), expected_len);

            let mut tuple = Tuple::empty();
            tuple.push_string(&test_string);
            let serialized = serialize_tuple(&tuple).unwrap();
            let deserialized = parse_tuple(&serialized).unwrap();
            assert_eq!(tuple, deserialized);

            if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
                let parsed = Tuple::parse_snake_string(slice);
                assert_eq!(parsed, Some(test_string.to_string()));
            } else {
                panic!("Expected slice item for string of length {}", expected_len);
            }
        }
    }

    #[test]
    fn test_utf8_strings() {
        let test_cases = vec![
            "Hello 世界".to_string(),                  // Mixed ASCII and Chinese
            "🚀 Rocket".to_string(),                   // Emoji
            "αβγδε".to_string(),                       // Greek letters
            "café".to_string(),                        // Accented characters
            "русский текст".to_string(),               // Cyrillic
            ("a".repeat(50) + "🚀" + &"b".repeat(50)), // Emoji in middle
        ];

        for test_string in test_cases {
            let mut tuple = Tuple::empty();
            tuple.push_string(&test_string);
            let serialized = serialize_tuple(&tuple).unwrap();
            let deserialized = parse_tuple(&serialized).unwrap();
            assert_eq!(tuple, deserialized);

            // Test that we can parse it back
            if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
                let parsed = Tuple::parse_snake_string(slice);
                assert_eq!(parsed, Some(test_string));
            } else {
                panic!("Expected slice item for UTF-8 string");
            }
        }
    }

    #[test]
    fn test_parse_snake_string_direct() {
        let test_strings = vec![
            "Hello".to_string(),
            "a".repeat(127),
            "a".repeat(200),
            "Test with spaces and symbols: !@#$%^&*()".to_string(),
        ];

        for original in test_strings {
            let mut tuple = Tuple::empty();
            tuple.push_string(&original);

            if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
                let parsed = Tuple::parse_snake_string(slice);
                assert_eq!(
                    parsed,
                    Some(original.clone()),
                    "Failed to parse: {}",
                    original
                );
            } else {
                panic!("Expected slice item");
            }
        }
    }

    #[test]
    fn test_push_string_direct() {
        let test_cases = vec![
            ("".to_string(), 0, false),  // empty string, 0 bits, fits in one cell
            ("x".to_string(), 8, false), // single char, 8 bits, fits in one cell
            ("Hello World".to_string(), 88, false), // short string, fits in one cell
            ("a".repeat(126), 1008, false), // exactly 126 bytes = 1008 bits, fits in one cell
            ("a".repeat(127), 1016, true), // 127 bytes = 1016 bits, requires multiple cells (max 126 per cell)
            ("a".repeat(128), 1024, true), // 128 bytes = 1024 bits, requires multiple cells
        ];

        for (test_string, expected_total_bits, requires_multiple_cells) in test_cases {
            let mut tuple = Tuple::empty();
            tuple.push_string(&test_string);

            assert_eq!(
                tuple.0.len(),
                1,
                "Expected exactly one tuple item for string: {}",
                test_string
            );

            let Some(TupleItem::Slice(cell)) = tuple.0.first() else {
                panic!("Expected slice item for string: {}", test_string);
            };

            let actual_bits = cell.bit_len();

            if requires_multiple_cells {
                assert_eq!(
                    actual_bits, 1008,
                    "First cell should contain 1008 bits for multi-cell string: {}",
                    test_string
                );
                assert_eq!(
                    cell.references().len(),
                    1,
                    "Multi-cell string should have 1 reference: {}",
                    test_string
                );
            } else {
                assert_eq!(
                    actual_bits, expected_total_bits,
                    "Bit count mismatch for single-cell string: {}",
                    test_string
                );
                assert_eq!(
                    cell.references().len(),
                    0,
                    "Single-cell string should have 0 references: {}",
                    test_string
                );
            }
        }
    }
}
