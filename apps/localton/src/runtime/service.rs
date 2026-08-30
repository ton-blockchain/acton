//! Implementation-independent lifecycle contract for long-running services.
//!
//! TON tool adapters may start an operating-system process today and an
//! in-process or controlled test implementation tomorrow. This module keeps the
//! Localton supervisor independent from that choice while retaining the small
//! set of lifecycle operations it actually needs.

use std::{fmt, process::ExitStatus};

use anyhow::Result;
use async_trait::async_trait;

/// Stable description of a service that has finished running.
///
/// The instance must be able to report exits from both operating-system
/// processes and non-process implementations. Keeping the rendered description
/// preserves useful platform-specific diagnostics, while `success` and `code`
/// provide structured fields for status APIs and tests. Implementations must not
/// include command lines, environment values, or other secrets in `description`
/// because it is safe to emit this value in lifecycle logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceExit {
    success: bool,
    code: Option<i32>,
    description: String,
}

impl ServiceExit {
    /// Creates an exit description for a process or process-independent service.
    ///
    /// `code` may be absent when a process was terminated by a signal or when an
    /// implementation has no numeric exit-code concept. `description` is the
    /// human-readable diagnostic and therefore must already be safe for logs.
    pub fn new(success: bool, code: Option<i32>, description: impl Into<String>) -> Self {
        Self {
            success,
            code,
            description: description.into(),
        }
    }

    /// Returns whether the implementation considers this a successful exit.
    ///
    /// A successful exit still means that a required long-running service is no
    /// longer alive; [`crate::runtime::ProcessRegistry`] intentionally treats any
    /// observed exit as an early-exit condition during supervision.
    pub fn success(&self) -> bool {
        self.success
    }

    /// Returns the numeric exit code when the implementation exposes one.
    pub fn code(&self) -> Option<i32> {
        self.code
    }
}

impl From<ExitStatus> for ServiceExit {
    fn from(status: ExitStatus) -> Self {
        Self::new(status.success(), status.code(), status.to_string())
    }
}

impl fmt::Display for ServiceExit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.description)
    }
}

/// Lifecycle operations required by Localton's service supervisor.
///
/// The trait is deliberately smaller than a subprocess API. Tool adapters own
/// startup arguments and readiness semantics; the runtime only needs identity,
/// liveness, and coordinated shutdown. The mutable receiver serializes status
/// inspection with shutdown and matches process APIs such as `try_wait`.
#[async_trait]
pub trait ManagedService: Send {
    /// Returns the stable service name used for uniqueness and diagnostics.
    fn name(&self) -> &str;

    /// Returns an operating-system PID when this implementation has one.
    ///
    /// In-process and controlled test implementations return `None`; callers must
    /// not use PID presence as the definition of service liveness.
    fn pid(&self) -> Option<u32>;

    /// Inspects the service without waiting for it to finish.
    ///
    /// `None` means the service remains alive. Once an exit is observed,
    /// implementations should continue returning the same outcome when practical
    /// so repeated health checks remain deterministic.
    fn try_status(&mut self) -> Result<Option<ServiceExit>>;

    /// Requests graceful shutdown and completes any required forced cleanup.
    ///
    /// Implementations must make this operation safe during instance teardown.
    /// In particular, subprocess implementations are responsible for descendants
    /// and must not leave orphan process groups behind when graceful stop fails.
    async fn stop(&mut self) -> Result<()>;
}

/// Type-erased ownership of one long-running service implementation.
///
/// The handle lets [`crate::runtime::ProcessRegistry`] supervise official TON
/// subprocesses and controlled implementations through the same lifecycle. It
/// owns the service so dropping the handle also triggers any implementation-level
/// safety behavior, such as [`crate::runtime::ManagedProcess`]'s kill-on-drop.
pub struct ServiceHandle {
    service: Box<dyn ManagedService>,
}

impl ServiceHandle {
    /// Erases a concrete managed service while retaining exclusive lifecycle
    /// ownership.
    pub fn new(service: impl ManagedService + 'static) -> Self {
        Self {
            service: Box::new(service),
        }
    }

    /// Returns the stable service name delegated to the underlying service.
    pub fn name(&self) -> &str {
        self.service.name()
    }

    /// Returns the underlying service PID when one exists.
    pub fn pid(&self) -> Option<u32> {
        self.service.pid()
    }

    /// Performs a non-blocking status inspection through the erased interface.
    pub fn try_status(&mut self) -> Result<Option<ServiceExit>> {
        self.service.try_status()
    }

    /// Stops the underlying service without exposing its concrete implementation.
    pub async fn stop(&mut self) -> Result<()> {
        self.service.stop().await
    }
}

impl fmt::Debug for ServiceHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceHandle")
            .field("name", &self.name())
            .field("pid", &self.pid())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_exit_keeps_structured_and_human_readable_outcome() {
        let exit = ServiceExit::new(false, Some(7), "exit status: 7");

        assert!(!exit.success());
        assert_eq!(exit.code(), Some(7));
        assert_eq!(exit.to_string(), "exit status: 7");
    }
}
