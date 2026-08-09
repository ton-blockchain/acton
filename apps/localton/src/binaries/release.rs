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
        sha256: "9ada018614dd095594429f7684109c8e8d9d97b664168ea0c9771dcd347889d5",
    })
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-mac-x86-64.zip",
        sha256: "90172ea443974847e667e1d05a925c8f51b5e7ff52b75bff3d89449bd48051f2",
    })
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-arm64.zip",
        sha256: "f93dd78d907d47507b3b41f74a4fcee5fefb5b164c3df1469d436100dfd87a7a",
    })
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-x86_64.zip",
        sha256: "15a252cbe49f700f52863a397c615283e150ec0fa72eb5d55893d3346bec8d04",
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
