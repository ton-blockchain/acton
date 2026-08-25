use acton_client::__private::tycho_types::cell::{CellBuilder, CellSlice};
use acton_client::{AbiError, AbiLoad, AbiStore, BigInt, register_custom_codec};
use expect_test::expect;

#[acton_client::contract(abi = "tests/fixtures/custom-codec.abi.json")]
mod generated {}

#[test]
fn custom_point_matches_upstream() {
    let point = generated::CustomPoint {
        x: BigInt::from(10),
        y: BigInt::from(20),
    };
    let missing = point
        .to_cell()
        .expect_err("unregistered codec must fail")
        .to_string();

    register_custom_codec::<generated::CustomPoint>(
        "CustomPoint",
        Some(
            |point: &generated::CustomPoint, builder: &mut CellBuilder| {
                acton_client::cell::store_fixed_int(builder, &point.x, 8, false)?;
                acton_client::cell::store_fixed_int(builder, &point.y, 8, false)
            },
        ),
        Some(|slice: &mut CellSlice<'_>| {
            Ok(generated::CustomPoint {
                x: acton_client::cell::load_fixed_int(slice, 8, false)?,
                y: acton_client::cell::load_fixed_int(slice, 8, false)?,
            })
        }),
    )
    .expect("custom codec must register");

    let cell = point.to_cell().expect("custom point must encode");
    let decoded = generated::CustomPoint::from_cell(&cell).expect("custom point must decode");
    let duplicate = register_custom_codec::<generated::CustomPoint>(
        "CustomPoint",
        None::<fn(&generated::CustomPoint, &mut CellBuilder) -> Result<(), AbiError>>,
        None::<fn(&mut CellSlice<'_>) -> Result<generated::CustomPoint, AbiError>>,
    )
    .expect_err("duplicate custom codec must fail")
    .to_string();

    expect![[r#"
        (
            "custom pack/unpack is not registered for `CustomPoint`",
            "x{0A14}",
            CustomPoint {
                x: 10,
                y: 20,
            },
            "custom pack/unpack for `CustomPoint` is already registered",
        )
    "#]]
    .assert_debug_eq(&(
        missing,
        format!(
            "x{{{:X}}}",
            cell.as_slice()
                .expect("cell must be readable")
                .display_data()
        ),
        decoded,
        duplicate,
    ));
}
