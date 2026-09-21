//! Bounded RLDP2 downloads on an existing ADNL identity.
//!
//! Concurrent queries are routed by transfer ID. Queries fit in one systematic
//! `RaptorQ` symbol; incoming answers can span several independently encoded parts.

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use everscale_network::{MessageSubscriber, SubscriberContext, adnl};
use everscale_raptorq::{
    EncodingPacket, ObjectTransmissionInformation, PayloadId, SourceBlockDecoder,
};
use tokio::{
    sync::mpsc,
    task::spawn_blocking,
    time::{Instant, MissedTickBehavior, interval, sleep_until, timeout_at},
};
use tracing::debug;

const SYMBOL_SIZE: u32 = 768;
const MAX_PART_SIZE: u32 = 2_000_000;
const MAX_PARTS: u32 = 64;

pub(crate) struct Client {
    pending: Mutex<HashMap<[u8; 32], Pending>>,
    completed: Mutex<HashMap<[u8; 32], Completed>>,
}

struct Completed {
    local: adnl::NodeIdShort,
    peer: adnl::NodeIdShort,
    parts: u32,
    expires: Instant,
}

struct Pending {
    local: adnl::NodeIdShort,
    peer: adnl::NodeIdShort,
    sender: mpsc::Sender<Packet>,
}

impl Client {
    /// Installs the custom-message handler before ADNL starts listening.
    pub(crate) fn new(adnl: &adnl::Node) -> Result<Arc<Self>> {
        let client = Arc::new(Self {
            pending: Mutex::new(HashMap::new()),
            completed: Mutex::new(HashMap::new()),
        });
        adnl.add_message_subscriber(client.clone())?;
        Ok(client)
    }

    /// Owns one bounded transfer; cancellation unregisters its packet queue.
    pub(crate) async fn query(
        &self,
        adnl: &adnl::Node,
        local: &adnl::NodeIdShort,
        peer: &adnl::NodeIdShort,
        data: Vec<u8>,
        timeout: Duration,
        max_answer_size: usize,
    ) -> Result<Vec<u8>> {
        let started = Instant::now();
        let deadline = started + timeout;
        let query_id = rand::random::<[u8; 32]>();
        let outgoing = rand::random::<[u8; 32]>();
        let incoming = outgoing.map(|byte| !byte);
        let unix_deadline = SystemTime::now().duration_since(UNIX_EPOCH)? + timeout;
        let mut query = tl_proto::serialize(Message::Query {
            query_id,
            max_answer_size: max_answer_size as u64,
            timeout: u32::try_from(unix_deadline.as_secs().saturating_add(1))?,
            data,
        });

        ensure!(
            query.len() <= SYMBOL_SIZE as usize,
            "RLDP2 query exceeds one source symbol"
        );
        let query_size = query.len() as u32;

        // RaptorQ is systematic: the first source symbol is the query padded
        // with zeroes. Repeating it is sufficient for these single-symbol RPCs.
        query.resize(SYMBOL_SIZE as usize, 0);
        let packet = tl_proto::serialize(Packet::Data {
            transfer_id: outgoing,
            fec_type: Fec {
                data_size: query_size,
                symbol_size: SYMBOL_SIZE,
                symbols_count: 1,
            },
            part: 0,
            total_size: u64::from(query_size),
            seqno: 0,
            data: query,
        });

        let (sender, mut receiver) = mpsc::channel(256);

        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| anyhow::anyhow!("RLDP2 registry poisoned"))?;
            ensure!(pending.len() < 256, "too many concurrent RLDP2 queries");
            ensure!(
                !pending.contains_key(&incoming),
                "RLDP2 transfer ID collision"
            );
            pending.insert(
                incoming,
                Pending {
                    local: *local,
                    peer: *peer,
                    sender,
                },
            );
        }

        let _registration = Registration {
            client: self,
            incoming,
            deadline,
        };
        let mut transfer = Transfer::new(max_answer_size + 64);
        let mut resend = interval(Duration::from_millis(250));
        resend.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut query_delivered = false;
        let mut query_sends = 0;
        let mut first_answer_at = None;
        let mut decoding = Duration::ZERO;

        loop {
            tokio::select! {
                _ = sleep_until(deadline) => {
                    bail!(
                        "RLDP2 query to {peer} timed out after {timeout:?}: query_delivered={query_delivered}, answer_parts={}",
                        transfer.parts.len()
                    );
                }
                _ = resend.tick(), if !query_delivered => {
                    adnl.send_custom_message(local, peer, &packet)?;
                    query_sends += 1;
                }
                packet = receiver.recv() => {
                    let packet = packet.context("RLDP2 packet queue closed")?;

                    match packet {
                        Packet::Complete { transfer_id, part: 0 } if transfer_id == outgoing => {
                            query_delivered = true;
                        }
                        Packet::Data { transfer_id, fec_type, part, total_size, seqno, data }
                            if transfer_id == incoming => {
                            query_delivered = true;
                            first_answer_at.get_or_insert_with(Instant::now);
                            let decode_started = Instant::now();
                            let ack = timeout_at(deadline, transfer.receive(
                                fec_type, part, total_size, seqno, data, incoming,
                            ))
                            .await
                            .with_context(|| format!("RLDP2 decoding from {peer} timed out after {timeout:?}"))??;
                            decoding += decode_started.elapsed();
                            adnl.send_custom_message(local, peer, &tl_proto::serialize(ack))?;

                            if let Some(bytes) = transfer.finish() {
                                let message = tl_proto::deserialize(&bytes)?;
                                let Message::Answer { query_id: answer_id, data } = message else {
                                    bail!("unexpected RLDP2 message type from {peer}");
                                };

                                ensure!(answer_id == query_id, "RLDP2 answer query ID mismatch");
                                ensure!(data.len() <= max_answer_size, "RLDP2 answer exceeds size limit");

                                debug!(
                                    operation = "rldp_download",
                                    node = %local,
                                    target = %peer,
                                    query_sends,
                                    first_answer_ms = first_answer_at.map(|at| at.duration_since(started).as_millis()),
                                    decode_ms = decoding.as_millis(),
                                    duration_ms = started.elapsed().as_millis(),
                                    outcome = "received",
                                    "completed RLDP2 answer",
                                );

                                return Ok(data);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

struct Registration<'a> {
    client: &'a Client,
    incoming: [u8; 32],
    deadline: Instant,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        let pending = self
            .client
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.incoming);
        let Some(pending) = pending else { return };

        // RLDP2 has no query cancellation message. Acknowledge late parts of a
        // discarded answer so a losing speculative request stops retransmitting.
        // Receipts are bound to the original peer and expire at the query deadline;
        // acknowledging discarded data does not make it a validated response.
        let mut completed = self
            .client
            .completed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        completed.retain(|_, item| item.expires > Instant::now());
        if completed.len() >= 1024
            && let Some(oldest) = completed
                .iter()
                .min_by_key(|(_, item)| item.expires)
                .map(|(id, _)| *id)
        {
            completed.remove(&oldest);
        }
        completed.insert(
            self.incoming,
            Completed {
                local: pending.local,
                peer: pending.peer,
                parts: MAX_PARTS,
                expires: self.deadline,
            },
        );
    }
}

#[async_trait::async_trait]
impl MessageSubscriber for Client {
    async fn try_consume_custom<'a>(
        &self,
        ctx: SubscriberContext<'a>,
        constructor: u32,
        data: &'a [u8],
    ) -> Result<bool> {
        const DATA: u32 = tl_proto::id!("rldp2.messagePart", scheme = "rldp.tl");
        const CONFIRM: u32 = tl_proto::id!("rldp2.confirm", scheme = "rldp.tl");
        const COMPLETE: u32 = tl_proto::id!("rldp2.complete", scheme = "rldp.tl");

        if !matches!(constructor, DATA | CONFIRM | COMPLETE) {
            return Ok(false);
        }

        if data.len() < 36 || data.len() > 2048 {
            return Ok(true);
        }
        let transfer_id: [u8; 32] = data[4..36].try_into()?;

        // Transfer IDs and part indices occupy fixed offsets in messagePart.
        // Inspect them without allocating or passing unsolicited FEC data to a decoder.
        if constructor == DATA && data.len() >= 56 {
            let completed = self
                .completed
                .lock()
                .map_err(|_| anyhow::anyhow!("RLDP2 receipt cache poisoned"))?;

            if let Some(receipt) = completed.get(&transfer_id) {
                if receipt.expires <= Instant::now()
                    || ctx.local_id != &receipt.local
                    || ctx.peer_id != &receipt.peer
                {
                    return Ok(true);
                }

                let Packet::Data {
                    transfer_id, part, ..
                } = tl_proto::deserialize(data)?
                else {
                    return Ok(true);
                };

                if part < receipt.parts {
                    let complete = tl_proto::serialize(Packet::Complete { transfer_id, part });
                    ctx.adnl
                        .send_custom_message(ctx.local_id, ctx.peer_id, &complete)?;
                }

                return Ok(true);
            }
        }

        // Discard unsolicited peers before allocating or decoding their payloads.
        let incoming = if constructor == DATA {
            transfer_id
        } else {
            transfer_id.map(|byte| !byte)
        };
        let sender = {
            let registry = self
                .pending
                .lock()
                .map_err(|_| anyhow::anyhow!("RLDP2 registry poisoned"))?;
            let Some(pending) = registry.get(&incoming) else {
                return Ok(true);
            };

            if ctx.local_id != &pending.local || ctx.peer_id != &pending.peer {
                return Ok(true);
            }

            let sender = pending.sender.clone();
            drop(registry);

            sender
        };

        let packet: Packet = tl_proto::deserialize(data)?;
        // Each query owns a bounded queue; overload cannot grow memory without limit.
        // RLDP2 repairs packets dropped while another transfer is being decoded.
        let _ = sender.try_send(packet);

        Ok(true)
    }
}

struct Transfer {
    limit: usize,
    total_size: Option<u64>,
    allocated: u64,
    parts: BTreeMap<u32, Part>,
}

struct Part {
    fec: Fec,
    decoder: Option<SourceBlockDecoder>,
    data: Option<Vec<u8>>,
    max_seqno: u32,
    received_mask: u32,
    received_count: u32,
}

impl Transfer {
    const fn new(limit: usize) -> Self {
        Self {
            limit,
            total_size: None,
            allocated: 0,
            parts: BTreeMap::new(),
        }
    }

    /// Validates FEC bounds before allocation, then acknowledges the received
    /// symbol. Different parts may arrive interleaved or out of order.
    /// Matrix reconstruction runs on the blocking pool so query deadlines remain responsive.
    async fn receive(
        &mut self,
        fec: Fec,
        index: u32,
        total_size: u64,
        seqno: u32,
        data: Vec<u8>,
        transfer_id: [u8; 32],
    ) -> Result<Packet> {
        ensure!(
            total_size > 0 && total_size <= self.limit as u64,
            "RLDP2 transfer exceeds size limit"
        );
        ensure!(
            self.total_size.is_none_or(|size| size == total_size),
            "RLDP2 transfer size changed"
        );
        ensure!(index < MAX_PARTS, "too many RLDP2 parts");
        ensure!(
            fec.data_size > 0 && fec.data_size <= MAX_PART_SIZE,
            "invalid RLDP2 part size"
        );
        ensure!(
            fec.symbol_size == SYMBOL_SIZE,
            "unsupported RLDP2 symbol size"
        );
        ensure!(
            fec.symbols_count == fec.data_size.div_ceil(SYMBOL_SIZE),
            "invalid RLDP2 symbol count"
        );
        ensure!(
            data.len() == SYMBOL_SIZE as usize && seqno < 1 << 24,
            "invalid RLDP2 symbol"
        );
        self.total_size = Some(total_size);

        if !self.parts.contains_key(&index) {
            ensure!(
                self.allocated + u64::from(fec.data_size) <= total_size,
                "RLDP2 parts exceed transfer size"
            );
            self.allocated += u64::from(fec.data_size);
            let config = ObjectTransmissionInformation::new(
                u64::from(fec.data_size),
                SYMBOL_SIZE as u16,
                1,
                1,
                1,
            );

            self.parts.insert(
                index,
                Part {
                    fec,
                    // TON uses P1 > P, while RFC 6330 permits P1 == P. The
                    // compatible codec is required to recover missing symbols.
                    decoder: Some(SourceBlockDecoder::new2(
                        0,
                        &config,
                        u64::from(fec.data_size),
                    )),
                    data: None,
                    max_seqno: 0,
                    received_mask: 0,
                    received_count: 0,
                },
            );
        }

        let part = self.parts.get_mut(&index).context("missing RLDP2 part")?;
        ensure!(part.fec == fec, "RLDP2 FEC parameters changed");

        if part.decoder.is_some() {
            // RLDP2 ACK numbers are one-based; RaptorQ symbol IDs are zero-based.
            let ack_seqno = seqno + 1;

            if ack_seqno > part.max_seqno {
                part.received_mask = part
                    .received_mask
                    .checked_shl(ack_seqno - part.max_seqno)
                    .unwrap_or(0);
                part.max_seqno = ack_seqno;
            }

            let distance = part.max_seqno - ack_seqno;

            if distance < 32 && part.received_mask & (1 << distance) == 0 {
                part.received_mask |= 1 << distance;
                part.received_count += 1;
                ensure!(
                    part.received_count <= fec.symbols_count * 4 + 64,
                    "RLDP2 repair budget exhausted"
                );
                let packet = EncodingPacket::new(PayloadId::new(0, seqno), data);
                let mut decoder = part.decoder.take().context("missing RLDP2 decoder")?;

                // Below K symbols the decoder only collects packets. At K it can
                // reconstruct a large matrix, which must not block the async task.
                let (decoder, decoded) =
                    if fec.symbols_count > 1 && part.received_count >= fec.symbols_count {
                        spawn_blocking(move || {
                            let decoded = decoder.decode([packet]);
                            (decoder, decoded)
                        })
                        .await
                        .context("RLDP2 decoder task failed")?
                    } else {
                        let decoded = decoder.decode([packet]);
                        (decoder, decoded)
                    };

                if let Some(mut bytes) = decoded {
                    bytes.truncate(fec.data_size as usize);
                    ensure!(
                        bytes.len() == fec.data_size as usize,
                        "RLDP2 decoded part size mismatch"
                    );
                    part.data = Some(bytes);
                } else {
                    part.decoder = Some(decoder);
                }
            }
        }

        Ok(if part.data.is_some() {
            Packet::Complete {
                transfer_id,
                part: index,
            }
        } else {
            Packet::Confirm {
                transfer_id,
                part: index,
                max_seqno: part.max_seqno,
                received_mask: part.received_mask,
                received_count: part.received_count,
            }
        })
    }

    fn finish(&mut self) -> Option<Vec<u8>> {
        if self.total_size != Some(self.allocated) || self.parts.is_empty() {
            return None;
        }

        for (index, (part_index, part)) in self.parts.iter().enumerate() {
            if *part_index != index as u32 || part.data.is_none() {
                return None;
            }
        }

        let mut bytes = Vec::with_capacity(self.allocated as usize);

        for part in self.parts.values_mut() {
            bytes.extend(part.data.take()?);
        }

        Some(bytes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, tl_proto::TlRead, tl_proto::TlWrite)]
#[tl(boxed, id = "fec.raptorQ", scheme = "rldp.tl")]
struct Fec {
    data_size: u32,
    symbol_size: u32,
    symbols_count: u32,
}

#[derive(tl_proto::TlRead, tl_proto::TlWrite)]
#[tl(boxed, scheme = "rldp.tl")]
enum Packet {
    #[tl(id = "rldp2.messagePart")]
    Data {
        transfer_id: [u8; 32],
        fec_type: Fec,
        part: u32,
        total_size: u64,
        seqno: u32,
        data: Vec<u8>,
    },
    #[tl(id = "rldp2.confirm")]
    Confirm {
        transfer_id: [u8; 32],
        part: u32,
        max_seqno: u32,
        received_mask: u32,
        received_count: u32,
    },
    #[tl(id = "rldp2.complete")]
    Complete { transfer_id: [u8; 32], part: u32 },
}

#[derive(tl_proto::TlRead, tl_proto::TlWrite)]
#[tl(boxed, scheme = "rldp.tl")]
enum Message {
    #[tl(id = "rldp.query")]
    Query {
        query_id: [u8; 32],
        max_answer_size: u64,
        timeout: u32,
        data: Vec<u8>,
    },
    #[tl(id = "rldp.answer")]
    Answer { query_id: [u8; 32], data: Vec<u8> },
}
