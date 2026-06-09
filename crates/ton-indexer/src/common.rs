use std::collections::HashMap;
use std::time::UNIX_EPOCH;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tvm_ffi::from_stack::FromStackTuple;
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub fn run_get_method<T: FromStackTuple>(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
    name: &str,
) -> anyhow::Result<T> {
    let result = run_get_method_raw(address, code, data, libs, name)?;
    T::from_tuple(result).map_err(Into::into)
}

fn run_get_method_raw(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
    name: &str,
) -> anyhow::Result<Tuple> {
    const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

    let method_id = (i32::from(CRC16.checksum(name.as_bytes())) & 0xFFFF) | 0x10000;

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: Boc::encode_base64(code),
        data: Boc::encode_base64(data),
        verbosity: ExecutorVerbosity::Short,
        libs: libs.unwrap_or_default().to_owned(),
        address,
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let executor = GetExecutor::new(&params)?;

    let stack = Tuple(vec![]);
    let stack = Boc::encode_base64(serialize_tuple(&stack)?);
    let result = executor.run_get_method(&stack, &params, None)?;

    match result {
        GetMethodResult::Success(result) => {
            if result.vm_exit_code != 0 {
                anyhow::bail!("VM exited with code {}", result.vm_exit_code);
            }

            let cell = Boc::decode_base64(result.stack.as_ref())?;

            Tuple::deserialize(&cell)
        }
        GetMethodResult::Error(error) => {
            anyhow::bail!("{}", error.error)
        }
    }
}
