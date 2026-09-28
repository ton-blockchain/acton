use std::borrow::Borrow;

use super::*;
use crate::prelude::*;

fn serialize_message<'a, T: Borrow<Message<'a>>>(message: T) -> Cell {
    CellBuilder::build_from(message.borrow()).unwrap()
}

fn check_message(boc: &[u8]) -> Cell {
    let boc = Boc::decode(boc).unwrap();
    let message = boc.parse::<Message>().unwrap();
    println!("message: {message:#?}");

    if let Some(init) = &message.init {
        let init = CellBuilder::build_from(init).unwrap();
        println!("{}", Boc::encode_base64(init));
    }

    let serialized = serialize_message(&message);
    assert_eq!(serialized.as_ref(), boc.as_ref());

    // Check an owned version
    {
        let owned = Lazy::<Message<'_>>::from_raw(boc.clone())
            .unwrap()
            .cast_into::<OwnedMessage>()
            .load()
            .unwrap();

        assert_eq!(CellBuilder::build_from(owned).unwrap(), boc);
    };

    boc
}

#[test]
fn external_message() -> anyhow::Result<()> {
    let boc = check_message(include_bytes!("external_message.boc"));
    assert_eq!(boc.parse::<MsgType>()?, MsgType::ExtIn);

    let body = Boc::decode(include_bytes!("external_message_body.boc")).unwrap();
    let serialized = serialize_message(Message {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: "0:8c8d0cc80ae34b93fe189fdefc0536745e40fab2a9179b37c24a419f04cd8e21"
                .parse()
                .unwrap(),
            ..Default::default()
        }),
        init: None,
        body: body.as_slice()?,
        layout: None,
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());

    Ok(())
}

#[test]
fn external_outgoing() {
    let boc = check_message(include_bytes!("external_out_message.boc"));
    assert_eq!(boc.parse::<MsgType>().unwrap(), MsgType::ExtOut);

    let body = Boc::decode_base64("te6ccgEBAQEADgAAGJMdgs1k/wsgCERmwQ==").unwrap();
    let serialized = serialize_message(Message {
        info: MsgInfo::ExtOut(ExtOutMsgInfo {
            src: "0:3addd84bf73267312a477049fd9b8db761bf39c585c150f8e6f9451347af2b6c"
                .parse()
                .unwrap(),
            dst: None,
            created_lt: 41854595000003,
            created_at: 1694436128,
        }),
        init: None,
        body: body.as_slice().unwrap(),
        layout: None,
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());
}

#[test]
fn internal_message_empty() {
    let boc = check_message(include_bytes!("empty_internal_message.boc"));
    assert_eq!(boc.parse::<MsgType>().unwrap(), MsgType::Int);

    let serialized = serialize_message(Message {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            src: "0:a921453472366b7feeec15323a96b5dcf17197c88dc0d4578dfa52900b8a33cb"
                .parse()
                .unwrap(),
            dst: "0:a921453472366b7feeec15323a96b5dcf17197c88dc0d4578dfa52900b8a33cb"
                .parse()
                .unwrap(),
            value: CurrencyCollection::new(1000000000),
            fwd_fee: Tokens::new(666672),
            created_lt: 34447525000002,
            created_at: 1673886009,
            ..Default::default()
        }),
        init: None,
        body: Default::default(),
        layout: None,
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());
}

#[test]
fn internal_message_with_body() -> anyhow::Result<()> {
    let boc = check_message(include_bytes!("internal_message_with_body.boc"));
    assert_eq!(boc.parse::<MsgType>().unwrap(), MsgType::Int);

    let body = Boc::decode(include_bytes!("internal_message_body.boc")).unwrap();

    let serialized = serialize_message(Message {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: true,
            src: "0:82615d4ce6bcd9989a82c9329f65569922f3437830eaa1003444b3fa4a46490f".parse()?,
            dst: "0:a732bba1c348ddae0970a541276e9cde4e44ac2c55e8079d034f88b0304f7c08".parse()?,
            value: CurrencyCollection::new(97621000),
            fwd_fee: Tokens::new(1586013),
            created_lt: 34447244000006,
            created_at: 1673885188,
            ..Default::default()
        }),
        init: None,
        body: body.as_slice()?,
        layout: Some(MessageLayout {
            init_to_cell: false,
            body_to_cell: true,
        }),
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());

    Ok(())
}

#[test]
fn internal_message_with_deploy() -> anyhow::Result<()> {
    let boc = check_message(include_bytes!("internal_message_with_deploy.boc"));
    assert_eq!(boc.parse::<MsgType>().unwrap(), MsgType::Int);

    let init = Boc::decode(include_bytes!(
        "internal_message_with_deploy_state_init.boc"
    ))
    .unwrap();
    let init = init.parse::<StateInit>().unwrap();

    let body = Boc::decode(include_bytes!("internal_message_with_deploy_body.boc")).unwrap();

    let serialized = serialize_message(Message {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: true,
            src: "0:098c37c0d8a78b32826de1d956242ee7830f83016eaa930e8c535295aea3ff1b".parse()?,
            dst: "0:a4232bb25ca73b09e1bb5200f87548f5a51a2d143d296a5a86b4bf74ec83e662".parse()?,
            value: CurrencyCollection::new(100000000),
            fwd_fee: Tokens::new(28859554),
            created_lt: 34447559000008,
            created_at: 1673886111,
            ..Default::default()
        }),
        init: Some(init),
        body: body.as_slice()?,
        layout: Some(MessageLayout {
            init_to_cell: true,
            body_to_cell: true,
        }),
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());

    Ok(())
}

#[test]
fn internal_message_with_deploy_special() -> anyhow::Result<()> {
    use crate::models::account::*;

    let boc = check_message(include_bytes!("internal_message_with_deploy_special.boc"));
    assert_eq!(boc.parse::<MsgType>().unwrap(), MsgType::Int);

    let init = StateInit {
        split_depth: None,
        special: Some(SpecialFlags {
            tick: true,
            tock: true,
        }),
        code: Some(Boc::decode_base64("te6ccgEBAQEABQAABv8AAA==")?),
        data: Some(Boc::decode_base64("te6ccgEBAQEABQAABv8AAA==")?),
        libraries: Default::default(),
    };

    let serialized = serialize_message(Message {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            src: "0:b62450b8355ae57d4e1530dda442e17dda60f39cee7cc0a34795566e30630dbf".parse()?,
            dst: "-1:a0b65eadaf741a132467f027eedc971a3d4f0d7ad34cc18edafac9d3c198fd9b".parse()?,
            value: CurrencyCollection::new(969351000),
            fwd_fee: Tokens::new(8206730),
            created_lt: 11129123000005,
            created_at: 1668282519,
            ..Default::default()
        }),
        init: Some(init),
        body: Default::default(),
        layout: Some(MessageLayout {
            init_to_cell: true,
            body_to_cell: false,
        }),
    });
    assert_eq!(boc.as_ref(), serialized.as_ref());

    Ok(())
}

#[test]
fn extra_flags_bit_len_same_as_tokens() {
    for bits in 0..=MessageExtraFlags::all().bits() {
        let flags = MessageExtraFlags::from_bits_retain(bits);
        assert_eq!(flags.bit_len(), flags.as_stored().unwrap_bit_len());
        assert_eq!(flags.bit_len(), Tokens::new(bits as _).unwrap_bit_len());
    }
}
