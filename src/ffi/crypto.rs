use crate::context::Context;
use crate::wallets::new_mnemonic;
use nacl::sign::{generate_keypair, signature};
use num_bigint::BigInt;
use rand::RngCore;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::cell::CellBuilder;

extension!(get_secure_random_bytes in (Context) with (bytes_num: BigInt) using get_secure_random_bytes_impl);
fn get_secure_random_bytes_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    bytes_num: BigInt,
) -> anyhow::Result<()> {
    let n: usize = bytes_num
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid bytesNum"))?;
    anyhow::ensure!(n > 0 && n <= 128, "bytesNum must be between 1 and 128");

    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);

    let mut builder = CellBuilder::new();
    builder.store_raw(&buf, (n * 8) as u16)?;
    let cell = builder.build()?;
    stack.push(TupleItem::Slice(cell));
    Ok(())
}

extension!(mnemonic_new in (Context) using mnemonic_new_impl);
fn mnemonic_new_impl(_ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    let words = new_mnemonic()?;
    let mut items = Tuple::empty();
    for word in &words {
        // Tolk `string` = Cell with a ref to a snake-string cell
        let mut snake = CellBuilder::new();
        snake.store_raw(word.as_bytes(), (word.len() * 8) as u16)?;
        let snake_cell = snake.build()?;

        let mut wrapper = CellBuilder::new();
        wrapper.store_reference(snake_cell)?;
        items.push(TupleItem::Cell(wrapper.build()?));
    }
    stack.push(TupleItem::Tuple(items));
    Ok(())
}

extension!(mnemonic_to_key_pair in (Context) with (words: Vec<String>) using mnemonic_to_key_pair_impl);
fn mnemonic_to_key_pair_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    words: Vec<String>,
) -> anyhow::Result<()> {
    let words = words.iter().map(String::as_str).collect();

    let mnemonic = tonlib_core::wallet::mnemonic::Mnemonic::new(words, &None)?;
    let key_pair = mnemonic.to_key_pair()?;

    // Return KeyPair { privateKey: bytes32, publicKey: bytes32 }
    // privateKey is the 32-byte seed (first 32 bytes of the 64-byte nacl secret key)
    // privateKey is the 32-byte seed (first 32 bytes of the 64-byte nacl secret key)
    let private_key = BigInt::from_bytes_be(num_bigint::Sign::Plus, &key_pair.secret_key[..32]);
    let public_key = BigInt::from_bytes_be(num_bigint::Sign::Plus, &key_pair.public_key);

    let mut result = Tuple::empty();
    result.push(TupleItem::Int(private_key));
    result.push(TupleItem::Int(public_key));

    stack.push(TupleItem::Tuple(result));
    Ok(())
}

extension!(raw_sign in (Context) with (data: BigInt, private_key: BigInt) using raw_sign_impl);
fn raw_sign_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    data: BigInt,
    private_key: BigInt,
) -> anyhow::Result<()> {
    // Convert private key (32-byte seed) to bytes
    let (_, pk_bytes) = private_key.to_bytes_be();
    let mut seed = [0u8; 32];
    let offset = 32usize.saturating_sub(pk_bytes.len());
    seed[offset..].copy_from_slice(&pk_bytes[..pk_bytes.len().min(32)]);

    // Derive full 64-byte nacl secret key from the 32-byte seed
    let keypair = generate_keypair(&seed);

    // Convert data (uint256) to 32 bytes
    let (_, data_bytes) = data.to_bytes_be();
    let mut hash = [0u8; 32];
    let offset = 32usize.saturating_sub(data_bytes.len());
    hash[offset..].copy_from_slice(&data_bytes[..data_bytes.len().min(32)]);

    // Sign the hash
    let sig = signature(&hash, &keypair.skey)
        .map_err(|e| anyhow::anyhow!("signing failed: {}", e.message))?;

    // Return signature as a 512-bit slice (64 bytes)
    let mut builder = CellBuilder::new();
    builder.store_raw(&sig, 512)?;
    let cell = builder.build()?;
    stack.push(TupleItem::Slice(cell));
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        400 => get_secure_random_bytes : 1,
        401 => mnemonic_new : 0,
        402 => mnemonic_to_key_pair : 1,
        403 => raw_sign : 2,
    });
}
