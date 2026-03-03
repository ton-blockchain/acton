use num_bigint::BigInt;
use tvmffi::stack::{Tuple, TupleItem};

#[test]
fn big_array_from_vec_small_values() {
    let big_array = TupleItem::big_array_from_vec(vec![1.into(), 2.into(), 3.into()]);
    let TupleItem::Tuple(big_array_fields) = big_array else {
        panic!("BigArray must be encoded as a tuple");
    };

    assert_eq!(big_array_fields.len(), 3);
    assert_eq!(big_array_fields[0], TupleItem::Int(BigInt::from(-1)));
    assert_eq!(big_array_fields[2], TupleItem::Int(BigInt::from(3)));

    let TupleItem::Tuple(top_level) = &big_array_fields[1] else {
        panic!("topLevel must be encoded as array<array<T>> tuple");
    };
    assert_eq!(top_level.len(), 255);

    let TupleItem::Tuple(first_bin) = &top_level[0] else {
        panic!("bin[0] must be an array tuple");
    };
    assert_eq!(
        first_bin,
        &Tuple(vec![
            TupleItem::Int(1.into()),
            TupleItem::Int(2.into()),
            TupleItem::Int(3.into())
        ])
    );

    for bin in top_level.iter().skip(1) {
        let TupleItem::Tuple(items) = bin else {
            panic!("all bins must be tuples");
        };
        assert!(items.is_empty());
    }
}

#[test]
fn big_array_from_vec_splits_into_bins_by_255_items() {
    let values = (0..260).map(BigInt::from).collect::<Vec<_>>();
    let big_array = TupleItem::big_array_from_vec(values);
    let TupleItem::Tuple(big_array_fields) = big_array else {
        panic!("BigArray must be encoded as a tuple");
    };

    let TupleItem::Tuple(top_level) = &big_array_fields[1] else {
        panic!("topLevel must be encoded as array<array<T>> tuple");
    };
    let TupleItem::Tuple(first_bin) = &top_level[0] else {
        panic!("bin[0] must be an array tuple");
    };
    let TupleItem::Tuple(second_bin) = &top_level[1] else {
        panic!("bin[1] must be an array tuple");
    };

    assert_eq!(first_bin.len(), 255);
    assert_eq!(second_bin.len(), 5);
    assert_eq!(second_bin[0], TupleItem::Int(255.into()));
    assert_eq!(second_bin[4], TupleItem::Int(259.into()));
    assert_eq!(big_array_fields[2], TupleItem::Int(BigInt::from(260)));
}

#[test]
#[should_panic(expected = "BigArray supports at most 65025 items")]
fn big_array_from_vec_panics_on_overflow() {
    let values = (0..65026).map(BigInt::from).collect::<Vec<_>>();
    let _ = TupleItem::big_array_from_vec(values);
}
