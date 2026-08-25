pub mod counter {
    include!(concat!(env!("OUT_DIR"), "/counter.rs"));
}

pub fn increase_body(
    query_id: u64,
    increase_by: u32,
) -> Result<acton_client::Cell, acton_client::AbiError> {
    acton_client::encode(&counter::IncreaseCounter {
        query_id: query_id.into(),
        increase_by: increase_by.into(),
    })
}
