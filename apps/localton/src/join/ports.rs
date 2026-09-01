//! Stable host-local port allocation for one joined TON node.
//!
//! A joined node advertises several ports in persistent validator-engine state.
//! Therefore automatic discovery is safe only during the first initialization.
//! The chosen ports are then stored in `settings.json` and reused verbatim.

use std::{
    net::{Ipv4Addr, TcpListener, UdpSocket},
    ops::RangeInclusive,
};

use anyhow::{Result, bail, ensure};

use crate::storage::NodePorts;

pub(super) const DEFAULT_JOIN_PORT_BASE: u16 = 19_000;
const PORTS_PER_INSTANCE: u16 = 6;

/// One contiguous host-local allocation for a joined node.
///
/// Keeping the ports adjacent makes host configuration predictable while allowing
/// the allocator to skip a range occupied by unrelated software.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HostPortAllocation {
    /// Public observability API and dashboard port
    pub observability: u16,
    /// Complete protocol port set for the joined node
    pub node: NodePorts,
    /// First reserved port in the contiguous range
    pub start: u16,
    /// Last reserved port in the contiguous range
    pub end: u16,
}

impl HostPortAllocation {
    /// Finds the first range at or above `first_candidate` that is free for both
    /// TCP and UDP. Both protocols are checked so numeric ranges do not overlap
    /// even though TON currently uses each individual port for only one protocol.
    pub fn find(first_candidate: u16) -> Result<Self> {
        Self::find_with(first_candidate, range_is_available)
    }

    fn find_with(
        first_candidate: u16,
        mut available: impl FnMut(RangeInclusive<u16>) -> bool,
    ) -> Result<Self> {
        ensure!(first_candidate > 0, "join port base must be positive");
        let last_candidate = u16::MAX
            .checked_sub(PORTS_PER_INSTANCE - 1)
            .ok_or_else(|| anyhow::anyhow!("join port range is too large"))?;

        // Shift by one port until the whole range is simultaneously available;
        // accepting separate per-node probes could persist overlapping ranges.
        for start in first_candidate..=last_candidate {
            let end = start + PORTS_PER_INSTANCE - 1;
            if available(start..=end) {
                return Ok(Self::at(start));
            }
        }

        bail!(
            "no contiguous range of {PORTS_PER_INSTANCE} TCP/UDP ports is available from {first_candidate}"
        )
    }

    fn at(start: u16) -> Self {
        Self {
            observability: start,
            node: NodePorts {
                console: start + 1,
                adnl: start + 2,
                liteserver: start + 3,
                out: start + 4,
                dht: start + 5,
            },
            start,
            end: start + PORTS_PER_INSTANCE - 1,
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
            HostPortAllocation::find_with(19_000, |range| *range.start() >= 19_003).unwrap();

        assert_eq!(allocation.start, 19_003);
        assert_eq!(allocation.end, 19_008);
        assert_eq!(allocation.observability, 19_003);
        assert_eq!(
            allocation.node,
            NodePorts {
                console: 19_004,
                adnl: 19_005,
                liteserver: 19_006,
                out: 19_007,
                dht: 19_008,
            }
        );
    }

    #[test]
    fn allocation_rejects_a_range_past_the_port_limit() {
        assert!(HostPortAllocation::find_with(u16::MAX, |_| true).is_err());
    }
}
