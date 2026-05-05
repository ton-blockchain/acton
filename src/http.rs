use std::env;
use std::ffi::OsStr;

pub(crate) const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";

pub(crate) fn blocking_client_builder() -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder();
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

#[cfg(test)]
mod tests {
    use super::proxy_enabled_from_value;
    use std::ffi::OsStr;

    #[test]
    fn acton_use_proxy_is_disabled_by_default() {
        assert!(!proxy_enabled_from_value(None));
    }

    #[test]
    fn acton_use_proxy_accepts_1_or_true() {
        for value in ["1", "true"] {
            assert!(proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }

    #[test]
    fn acton_use_proxy_rejects_other_values() {
        for value in ["", "0", "false", "TRUE", "yes"] {
            assert!(!proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }
}
