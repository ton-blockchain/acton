//! Local TON network startup and lifecycle management.
//!
//! The bootstrap pipeline prepares persistent state, creates a genesis when
//! necessary, starts TON processes, waits for block production, exposes the
//! configured HTTP services, and performs an orderly shutdown. Each technical
//! part of that sequence lives in a focused submodule so the top-level pipeline
//! remains readable.

use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use fs2::FileExt;

mod dht;
mod files;
mod genesis;
mod pipeline;
mod readiness;
mod zerostate;

pub use pipeline::run;
pub(crate) use readiness::{shutdown_signal, supervise};

/// Locks a state directory for the lifetime of one Localton instance.
///
/// Two instances sharing databases and fixed ports would corrupt runtime state
/// and compete for the same sockets. The returned open file owns the advisory
/// lock; dropping it releases the directory for the next invocation.
pub(crate) fn acquire_lock(path: &Path) -> Result<File> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open state lock {}", path.display()))?;
    file.try_lock_exclusive().with_context(|| {
        format!(
            "another localton process is already using {}",
            path.parent().unwrap_or(path).display()
        )
    })?;
    Ok(file)
}
