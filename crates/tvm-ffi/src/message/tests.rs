use super::original_message_body;
use tycho_types::cell::{Cell, CellBuilder};

#[test]
fn unwraps_only_the_selected_bounce_envelope_and_preserves_payload_refs() {
    let mut payload = CellBuilder::new();
    payload.store_u32(0x1234_5678).unwrap();
    payload.store_reference(Cell::default()).unwrap();
    let payload = payload.build().unwrap();

    let mut legacy = CellBuilder::new();
    legacy.store_u32(0xffff_ffff).unwrap();
    legacy.store_slice(payload.as_slice().unwrap()).unwrap();
    let legacy = legacy.build().unwrap();

    let mut original_info = CellBuilder::new();
    original_info.store_zeros(101).unwrap(); // zero coins, empty extra currencies, LT and time
    let mut rich = CellBuilder::new();
    rich.store_u32(0xffff_fffe).unwrap();
    rich.store_reference(payload.clone()).unwrap();
    rich.store_reference(original_info.build().unwrap())
        .unwrap();
    rich.store_u8(0).unwrap();
    rich.store_u32((-14_i32) as u32).unwrap();
    rich.store_bit_zero().unwrap();
    let rich = rich.build().unwrap();

    let mut custom = CellBuilder::new();
    custom.store_u32(0xdead_beef).unwrap();
    custom.store_slice(payload.as_slice().unwrap()).unwrap();
    let custom = custom.build().unwrap();

    for (body, bounced, expected) in [
        (&payload, false, &payload),
        (&legacy, false, &legacy),
        (&legacy, true, &payload),
        (&rich, false, &rich),
        (&rich, true, &payload),
        (&custom, true, &payload),
    ] {
        let actual = original_message_body(body.as_slice().unwrap(), bounced).unwrap();
        assert_eq!(CellBuilder::build_from(actual).unwrap(), *expected);
    }

    // A payload that itself starts with a bounce tag is not recursively unwrapped.
    let mut nested = CellBuilder::new();
    nested.store_u32(0xffff_fffe).unwrap();
    nested.store_reference(legacy.clone()).unwrap();
    let nested = nested.build().unwrap();
    let actual = original_message_body(nested.as_slice().unwrap(), true).unwrap();
    assert_eq!(CellBuilder::build_from(actual).unwrap(), legacy);
}

#[test]
fn missing_rich_payload_never_falls_back_to_diagnostics() {
    let mut body = CellBuilder::new();
    body.store_u32(0xffff_fffe).unwrap();
    body.store_u32(0x1234_5678).unwrap();
    let body = body.build().unwrap();
    assert!(original_message_body(body.as_slice().unwrap(), true).is_none());
}

#[test]
fn short_original_bodies_have_no_opcode() {
    for bit_count in [0, 3, 31] {
        let mut payload = CellBuilder::new();
        payload.store_zeros(bit_count).unwrap();
        let payload = payload.build().unwrap();

        let mut rich = CellBuilder::new();
        rich.store_u32(0xffff_fffe).unwrap();
        rich.store_reference(payload.clone()).unwrap();
        let rich = rich.build().unwrap();

        assert!(original_message_body(payload.as_slice().unwrap(), true).is_none());
        let mut original = original_message_body(rich.as_slice().unwrap(), true).unwrap();
        assert_eq!(original.size_bits(), bit_count);
        assert!(original.load_u32().is_err());
    }
}
