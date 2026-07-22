use crate::storage::BlockMeta;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeClockInfo {
    pub current_unix_time: u32,
    pub time_offset_seconds: i64,
    pub next_block_timestamp: Option<u32>,
}

pub(crate) struct VirtualClock {
    offset_seconds: i64,
    next_block_timestamp: Option<u32>,
}

impl VirtualClock {
    pub(crate) fn from_blocks(blocks: &[BlockMeta]) -> anyhow::Result<Self> {
        Ok(Self {
            offset_seconds: initial_time_offset_for_blocks(blocks)?,
            next_block_timestamp: None,
        })
    }

    pub(crate) const fn from_parts(offset_seconds: i64, next_block_timestamp: Option<u32>) -> Self {
        Self {
            offset_seconds,
            next_block_timestamp,
        }
    }

    pub(crate) const fn offset_seconds(&self) -> i64 {
        self.offset_seconds
    }

    pub(crate) const fn next_block_timestamp(&self) -> Option<u32> {
        self.next_block_timestamp
    }

    pub(crate) fn now_unix(&self) -> anyhow::Result<u32> {
        unix_now_with_offset(self.offset_seconds)
    }

    pub(crate) fn clock_info(&self) -> anyhow::Result<NodeClockInfo> {
        Ok(NodeClockInfo {
            current_unix_time: self.now_unix()?,
            time_offset_seconds: self.offset_seconds,
            next_block_timestamp: self.next_block_timestamp,
        })
    }

    pub(crate) fn increase_time(&mut self, seconds: u64) -> anyhow::Result<NodeClockInfo> {
        anyhow::ensure!(seconds > 0, "seconds must be greater than 0");
        let current = u64::from(self.now_unix()?);
        let next = current
            .checked_add(seconds)
            .context("localnet time overflow")?;
        anyhow::ensure!(
            next <= u64::from(u32::MAX),
            "localnet time cannot exceed {}",
            u32::MAX
        );
        let seconds = i64::try_from(seconds).context("localnet time delta is too large")?;
        self.offset_seconds = self
            .offset_seconds
            .checked_add(seconds)
            .context("localnet time offset overflow")?;
        self.clock_info()
    }

    pub(crate) fn set_time(
        &mut self,
        timestamp: u32,
        latest_block_timestamp: u32,
    ) -> anyhow::Result<NodeClockInfo> {
        ensure_timestamp_not_before_latest_block(timestamp, latest_block_timestamp)?;
        self.offset_seconds = i64::from(timestamp) - system_unix_now_i64()?;
        self.clock_info()
    }

    pub(crate) fn set_next_block_timestamp(
        &mut self,
        timestamp: u32,
        latest_block_timestamp: u32,
    ) -> anyhow::Result<NodeClockInfo> {
        ensure_timestamp_not_before_latest_block(timestamp, latest_block_timestamp)?;
        self.next_block_timestamp = Some(timestamp);
        self.clock_info()
    }

    pub(crate) fn next_block_gen_utime(
        &mut self,
        latest_block_timestamp: u32,
    ) -> anyhow::Result<u32> {
        if let Some(timestamp) = self.next_block_timestamp {
            ensure_timestamp_not_before_latest_block(timestamp, latest_block_timestamp)?;
            self.next_block_timestamp = None;
            self.bump_offset_to_at_least(timestamp)?;
            return Ok(timestamp);
        }

        self.now_unix()
    }

    pub(crate) fn bump_offset_to_at_least(&mut self, timestamp: u32) -> anyhow::Result<()> {
        let required = i64::from(timestamp) - system_unix_now_i64()?;
        if self.offset_seconds < required {
            self.offset_seconds = required;
        }
        Ok(())
    }
}

fn ensure_timestamp_not_before_latest_block(
    timestamp: u32,
    latest_block_timestamp: u32,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        timestamp >= latest_block_timestamp,
        "timestamp {timestamp} is before latest block timestamp {latest_block_timestamp}"
    );
    Ok(())
}

fn initial_time_offset_for_blocks(blocks: &[BlockMeta]) -> anyhow::Result<i64> {
    let Some(latest_block) = blocks.last() else {
        return Ok(0);
    };
    let required = i64::from(latest_block.gen_utime) - system_unix_now_i64()?;
    Ok(required.max(0))
}

fn unix_now_with_offset(offset_seconds: i64) -> anyhow::Result<u32> {
    let now = system_unix_now_i64()?
        .checked_add(offset_seconds)
        .context("localnet time offset overflow")?;
    anyhow::ensure!(now >= 0, "localnet time cannot be before unix epoch");
    u32::try_from(now).context("localnet time cannot exceed u32::MAX")
}

fn system_unix_now_i64() -> anyhow::Result<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).context("system unix time is too large")
}
