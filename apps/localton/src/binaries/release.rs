//! Platform mapping for the pinned official TON release assets.
//!
//! Each supported OS and CPU architecture maps to one archive filename and its
//! expected SHA-256 digest. Automatic installation is available for macOS and
//! Linux on arm64 and x86_64; other platforms must provide `--ton-bin-dir`.

use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub(super) struct ReleaseAsset {
    pub file_name: &'static str,
    pub sha256: &'static str,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-mac-arm64.zip",
        sha256: "46cb42491213adf2adc9cb7e94c922249a586ba55b7dfe4e66626695a68b9d4b",
    })
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-mac-x86-64.zip",
        sha256: "479cfb6ac9a750683816c1db7501bdc6d2d1d5fdb9a11861b784efc4f734be2a",
    })
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-arm64.zip",
        sha256: "acb73cd85118754e149744d35c3ef4775f065c8545b49600f70ffe8dea68060e",
    })
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-x86_64.zip",
        sha256: "548f0f29b5ebf1a42d6aa23438b25c7a0c05b2330a7f278d2ca83fc2015685e9",
    })
}

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
)))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    anyhow::bail!("automatic TON binary installation supports macOS/Linux on arm64/x86_64")
}

pub(super) fn platform_id() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_asset_has_sha256() {
        let asset = current_asset().unwrap();
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
