use std::sync::{Arc, LazyLock};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily};
use tycho_types::dict::Dict;

pub const DEFAULT_CONFIG: &str = include_str!("default_config.boc64");

pub static DEFAULT_CONFIG_CELL: LazyLock<Cell> = LazyLock::new(|| {
    Boc::decode_base64(DEFAULT_CONFIG).expect("constant config must be valid BoC")
});

pub static DEFAULT_CONFIG_DICT: LazyLock<Arc<Dict<u32, Cell>>> = LazyLock::new(|| {
    let mut slice = DEFAULT_CONFIG_CELL.as_slice_allow_exotic();
    Arc::new(
        Dict::load_from_root_ext(&mut slice, Cell::empty_context())
            .expect("constant config must be valid Dict"),
    )
});

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::{Context, Result, bail};
    use serde::Deserialize;

    use super::{Boc, DEFAULT_CONFIG};

    const TONCENTER_GET_CONFIG_ALL_URL: &str = "https://toncenter.com/api/v2/getConfigAll";
    const HTTP_CONNECT_TIMEOUT_SECS: u64 = 3;
    const HTTP_REQUEST_TIMEOUT_SECS: u64 = 8;

    #[derive(Deserialize)]
    struct TonCenterConfigAllResponse {
        ok: bool,
        result: TonCenterConfigInfo,
    }

    #[derive(Deserialize)]
    struct TonCenterConfigInfo {
        config: TonCenterConfigCell,
    }

    #[derive(Deserialize)]
    struct TonCenterConfigCell {
        bytes: String,
    }

    #[test]
    fn bundled_default_config_matches_toncenter_when_available() -> Result<()> {
        let remote_config = match fetch_toncenter_default_config() {
            Ok(config) => config,
            Err(error) => {
                eprintln!("skipping bundled default config TonCenter check: {error:#}");
                return Ok(());
            }
        };

        if remote_config != DEFAULT_CONFIG {
            bail!("bundled default config is out of date; run `cargo xtask update-default-config`");
        }

        Ok(())
    }

    fn fetch_toncenter_default_config() -> Result<String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()
            .context("failed to create TonCenter HTTP client")?;

        let response = client
            .get(TONCENTER_GET_CONFIG_ALL_URL)
            .send()
            .context("failed to send TonCenter getConfigAll request")?;
        let status = response.status();

        if !status.is_success() {
            bail!("TonCenter getConfigAll request failed with status {status}");
        }

        let response: TonCenterConfigAllResponse = response
            .json()
            .context("failed to parse TonCenter getConfigAll response JSON")?;

        if !response.ok {
            bail!("TonCenter returned ok=false for getConfigAll");
        }

        Boc::decode_base64(&response.result.config.bytes)
            .context("TonCenter getConfigAll config bytes are not a valid BOC")?;

        Ok(response.result.config.bytes)
    }
}
