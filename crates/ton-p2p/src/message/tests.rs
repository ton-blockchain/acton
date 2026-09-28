use anyhow::Result;
use expect_test::expect;
use rston::cell::CellBuilder;
use rston::models::{ExtInMsgInfo, IntMsgInfo};

use super::*;

#[test]
fn broadcast_packets_preserve_data_and_recover_a_missing_fec_symbol() -> Result<()> {
    use everscale_network::proto;
    use everscale_raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation, PayloadId};
    use tl_proto_network::TlRead;

    let keys = adnl::Keystore::builder()
        .with_tagged_key([7; 32], 0)?
        .build();
    let key = keys.key_by_tag(0)?;
    let overlay = overlay::IdFull::for_workchain_overlay(0, &[3; 32]).compute_short_id();
    let mut outcomes = Vec::new();

    for size in [768, 8192, 65_544] {
        let data = vec![42; size];
        let packets = broadcast::encode(key, &overlay, &data)?;
        let mut decoder = Decoder::new(ObjectTransmissionInformation::new(
            size as u64,
            768,
            1,
            1,
            1,
        ));
        let mut recovered = None;

        for packet in &packets {
            let mut offset = 0;
            let prefix = proto::overlay::Message::read_from(packet, &mut offset)?;
            ensure!(prefix.overlay == overlay.as_slice(), "wrong overlay");

            match proto::overlay::Broadcast::read_from(packet, &mut offset)? {
                proto::overlay::Broadcast::Broadcast(message) => {
                    ensure!(message.flags == 0, "AnySender is forbidden");
                    recovered = Some(message.data.to_vec());
                }
                proto::overlay::Broadcast::BroadcastFec(message) => {
                    ensure!(message.flags == 0, "AnySender is forbidden");
                    if message.seqno != 0 && recovered.is_none() {
                        recovered = decoder.decode(EncodingPacket::new(
                            PayloadId::new(0, message.seqno),
                            message.data.to_vec(),
                        ));
                    }
                }
                _ => bail!("unexpected broadcast type"),
            }
            ensure!(offset == packet.len(), "unconsumed packet bytes");
        }

        outcomes.push(format!(
            "bytes={size}, packets={}, recovered={}",
            packets.len(),
            recovered == Some(data)
        ));
    }

    expect![[r"
        bytes=768, packets=1, recovered=true
        bytes=8192, packets=17, recovered=true
        bytes=65544, packets=129, recovered=true"]]
    .assert_eq(&outcomes.join("\n"));
    Ok(())
}

#[test]
fn validates_message_kind_and_preserves_the_signed_boc() -> Result<()> {
    let mut outcomes = Vec::new();
    for workchain in [-1, 0, 1] {
        let destination = StdAddr::new(workchain, HashBytes([7; 32]));
        let message = OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                dst: destination.clone().into(),
                ..Default::default()
            }),
            init: None,
            body: CellBuilder::build_from(123_u32)?.into(),
            layout: None,
        };
        let root = CellBuilder::build_from(message)?;
        let boc = Boc::encode(&root);
        outcomes.push(match ExternalMessage::new(boc.clone()) {
            Ok(message) => format!(
                "{workchain}: destination={}, hash={}, boc={}",
                message.destination() == &destination,
                message.hash() == *root.repr_hash(),
                message.boc == boc,
            ),
            Err(error) => format!("{workchain}: {error}"),
        });
    }

    let internal = CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo::default()),
        init: None,
        body: CellBuilder::build_from(123_u32)?.into(),
        layout: None,
    })?;
    for bytes in [
        Boc::encode(internal),
        vec![0; ExternalMessage::MAX_BYTES + 1],
        vec![0],
    ] {
        outcomes.push(ExternalMessage::new(bytes).err().unwrap().to_string());
    }

    expect![[r"
        -1: destination=true, hash=true, boc=true
        0: destination=true, hash=true, boc=true
        1: external message destination must be in masterchain or basechain
        expected an inbound external message
        external message exceeds 65535 bytes
        invalid external message BoC"]]
    .assert_eq(&outcomes.join("\n"));
    Ok(())
}

#[test]
fn external_broadcast_uses_a_bare_nested_message() {
    // TON's tonNode.externalMessageBroadcast has a boxed outer constructor,
    // followed directly by tonNode.externalMessage.data as a TL bytes field.
    let bytes = tl_proto::serialize(ExternalMessageBroadcast {
        data: vec![1, 2, 3],
    });
    expect![[r"
        [
            103,
            24,
            27,
            61,
            3,
            1,
            2,
            3,
        ]
    "]]
    .assert_debug_eq(&bytes);
}
