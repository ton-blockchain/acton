use std::fmt;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;

pub const CORE_TARGET: &str = "ton_language_server_core";
pub const SERVICE_TARGET: &str = "ton_language_server_core::service";
pub const EDIT_TARGET: &str = "ton_language_server_core::edit";
pub const TLB_TARGET: &str = "ton_language_server_core::languages::tlb";
pub const TASM_TARGET: &str = "ton_language_server_core::languages::tasm";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    #[must_use]
    pub const fn as_tracing_level_filter(self) -> LevelFilter {
        match self {
            Self::Off => LevelFilter::OFF,
            Self::Error => LevelFilter::ERROR,
            Self::Warn => LevelFilter::WARN,
            Self::Info => LevelFilter::INFO,
            Self::Debug => LevelFilter::DEBUG,
            Self::Trace => LevelFilter::TRACE,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseLogLevelError {
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseLogLevelError {
    value: String,
}

impl fmt::Display for ParseLogLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown log level '{}'", self.value)
    }
}

impl std::error::Error for ParseLogLevelError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggingConfig {
    pub target: String,
    pub level: LogLevel,
}

impl LoggingConfig {
    #[must_use]
    pub fn new(level: LogLevel) -> Self {
        Self {
            target: CORE_TARGET.to_owned(),
            level,
        }
    }

    #[must_use]
    pub fn for_target(target: impl Into<String>, level: LogLevel) -> Self {
        Self {
            target: target.into(),
            level,
        }
    }

    #[must_use]
    pub fn filter_directive(&self) -> String {
        if self.target.is_empty() {
            self.level.to_string()
        } else {
            format!("{}={}", self.target, self.level)
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self::new(LogLevel::Info)
    }
}
