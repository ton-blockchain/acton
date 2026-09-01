//! Stable host-local port allocation for joined TON nodes.
//!
//! A follower advertises several ports in persistent validator-engine state.
//! Therefore automatic discovery is safe only during the first initialization.
//! The chosen ports are then stored in `settings.json` and reused verbatim.

use std::{
    net::{Ipv4Addr, TcpListener, UdpSocket},
    ops::RangeInclusive,
};

use anyhow::{Result, bail, ensure};

use crate::storage::NodePorts;

pub(super) const DEFAULT_JOIN_PORT_BASE: u16 = 19_000;
const PORTS_PER_NODE: u16 = 5;

/// One contiguous host-local allocation for joined nodes.
///
/// Keeping the ports adjacent makes multi-host local networks predictable while
/// still allowing the allocator to skip a range occupied by unrelated software.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostPortAllocation {
    pub nodes: Vec<NodePorts>,
    pub start: u16,
    pub end: u16,
}

impl HostPortAllocation {
    /// Finds the first range at or above `first_candidate` that is free for both
    /// TCP and UDP. Both protocols are checked so numeric ranges do not overlap
    /// even though TON currently uses each individual port for only one protocol.
    pub fn find(first_candidate: u16, node_count: usize) -> Result<Self> {
        Self::find_with(first_candidate, node_count, range_is_available)
    }

    fn find_with(
        first_candidate: u16,
        node_count: usize,
        mut available: impl FnMut(RangeInclusive<u16>) -> bool,
    ) -> Result<Self> {
        ensure!(first_candidate > 0, "join port base must be positive");
        ensure!(
            node_count > 0,
            "join must allocate ports for at least one node"
        );
        let node_count = u16::try_from(node_count).map_err(|_| {
            anyhow::anyhow!("joined node count does not fit the TCP/UDP port space")
        })?;
        let width = node_count
            .checked_mul(PORTS_PER_NODE)
            .ok_or_else(|| anyhow::anyhow!("join port range is too large"))?;
        let last_candidate = u16::MAX
            .checked_sub(width - 1)
            .ok_or_else(|| anyhow::anyhow!("join port range is too large"))?;

        for start in first_candidate..=last_candidate {
            let end = start + width - 1;
            if available(start..=end) {
                return Ok(Self::at(start, node_count));
            }
        }
        bail!("no contiguous range of {width} TCP/UDP ports is available from {first_candidate}")
    }

    fn at(start: u16, node_count: u16) -> Self {
        let nodes = (0..node_count)
            .map(|index| {
                let start = start + index * PORTS_PER_NODE;
                NodePorts {
                    console: start,
                    adnl: start + 1,
                    liteserver: start + 2,
                    out: start + 3,
                    dht: start + 4,
                }
            })
            .collect();
        Self {
            nodes,
            start,
            end: start + node_count * PORTS_PER_NODE - 1,
        }
    }
}

/// Probes an entire numeric range while retaining every temporary socket until
/// the probe completes. This prevents the probe itself from accepting a range
/// whose later ports collide with sockets opened for its earlier ports.
fn range_is_available(range: RangeInclusive<u16>) -> bool {
    let mut tcp = Vec::new();
    let mut udp = Vec::new();
    for port in range {
        let address = (Ipv4Addr::UNSPECIFIED, port);
        let Ok(listener) = TcpListener::bind(address) else {
            return false;
        };
        let Ok(socket) = UdpSocket::bind(address) else {
            return false;
        };
        tcp.push(listener);
        udp.push(socket);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_skips_to_the_first_contiguous_range() {
        let allocation =
            HostPortAllocation::find_with(19_000, 2, |range| *range.start() >= 19_003).unwrap();

        assert_eq!(allocation.start, 19_003);
        assert_eq!(allocation.end, 19_012);
        assert_eq!(
            allocation.nodes,
            vec![
                NodePorts {
                    console: 19_003,
                    adnl: 19_004,
                    liteserver: 19_005,
                    out: 19_006,
                    dht: 19_007,
                },
                NodePorts {
                    console: 19_008,
                    adnl: 19_009,
                    liteserver: 19_010,
                    out: 19_011,
                    dht: 19_012,
                },
            ]
        );
    }

    #[test]
    fn allocation_rejects_a_range_past_the_port_limit() {
        assert!(HostPortAllocation::find_with(u16::MAX, 1, |_| true).is_err());
    }
}
