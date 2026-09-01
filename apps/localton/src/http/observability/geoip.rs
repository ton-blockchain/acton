//! Country-level geolocation for public node addresses.
//!
//! The resolver downloads one versioned DB-IP Lite database into the shared
//! Localton cache. Individual node addresses never leave the host.

use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use flate2::read::GzDecoder;
use fs2::FileExt;
use maxminddb::{Reader, geoip2};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::{cache, observability::NodeLocation};

const DATABASE_RELEASE: &str = "2026-09";
const DATABASE_URL: &str = "https://download.db-ip.com/free/dbip-country-lite-2026-09.mmdb.gz";
const ARCHIVE_SHA256: &str = "cb0578ce59f569f2c933bb40feb820804a334855a60739011b0a89cab1d6e4ed";
const DATABASE_SHA256: &str = "d284ae2e7427fe33d83465e1506b2b21aae47eb8a9b099f8f4dac6a98c99f041";

/// Resolves public IPv4 addresses without sending lookup queries to a third party.
///
/// The reader owns the immutable database bytes, so lookups are synchronous and
/// can be shared by all requests for the lifetime of the observability service.
pub(super) struct GeoIpResolver {
    reader: Reader<Vec<u8>>,
}

impl GeoIpResolver {
    /// Loads the pinned country database, downloading and atomically caching it when absent.
    pub(super) async fn load() -> Result<Self> {
        let started_at = Instant::now();
        let path = database_path()?;
        let result = Self::load_inner(&path).await;
        let duration_ms = started_at.elapsed().as_millis();

        match &result {
            Ok(_) => info!(
                operation = "geoip_database_load",
                target = %path.display(),
                duration_ms,
                outcome = "success",
                "GeoIP country database ready"
            ),
            Err(error) => warn!(
                operation = "geoip_database_load",
                target = %path.display(),
                duration_ms,
                outcome = "unavailable",
                %error,
                "GeoIP country database is unavailable"
            ),
        }

        result
    }

    /// Returns country metadata only for globally routable node addresses.
    pub(super) fn locate(&self, value: &str) -> NodeLocation {
        let NodeAddress::Public(ip) = node_address(value) else {
            return location_without_database(value);
        };
        let Some(record) = self
            .reader
            .lookup(IpAddr::V4(ip))
            .ok()
            .and_then(|lookup| lookup.decode::<geoip2::Country>().ok())
            .flatten()
        else {
            return NodeLocation::Unavailable;
        };
        let Some(country_code) = record.country.iso_code.map(str::to_ascii_uppercase) else {
            return NodeLocation::Unavailable;
        };
        let country = record
            .country
            .names
            .english
            .unwrap_or(&country_code)
            .to_owned();

        NodeLocation::Country {
            country_code,
            country,
        }
    }

    async fn load_inner(path: &Path) -> Result<Self> {
        if let Ok(reader) = Reader::open_readfile(path) {
            return Ok(Self { reader });
        }

        let directory = path
            .parent()
            .context("GeoIP database path has no parent directory")?;
        fs::create_dir_all(directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;

        let _lock = acquire_install_lock(&directory.join("install.lock")).await?;

        if let Ok(reader) = Reader::open_readfile(path) {
            return Ok(Self { reader });
        }

        if path.exists() {
            warn!(
                target = %path.display(),
                "replacing invalid cached GeoIP country database"
            );
            fs::remove_file(path)
                .with_context(|| format!("failed to remove invalid {}", path.display()))?;
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to create GeoIP download client")?;
        let archive = client
            .get(DATABASE_URL)
            .send()
            .await
            .context("failed to download GeoIP country database")?
            .error_for_status()
            .context("GeoIP country database download failed")?
            .bytes()
            .await
            .context("failed to read GeoIP country database download")?;
        let target = path.to_owned();

        tokio::task::spawn_blocking(move || install_database(&target, &archive))
            .await
            .context("GeoIP database installation task failed")??;

        let reader = Reader::open_readfile(path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        Ok(Self { reader })
    }
}

fn database_path() -> Result<PathBuf> {
    Ok(cache::root()?
        .join("geoip")
        .join(format!("dbip-country-lite-{DATABASE_RELEASE}.mmdb")))
}

async fn acquire_install_lock(path: &Path) -> Result<File> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("failed to open GeoIP cache lock {}", path.display()))?;
        FileExt::lock_exclusive(&file)
            .with_context(|| format!("failed to lock GeoIP cache {}", path.display()))?;
        Ok(file)
    })
    .await
    .context("GeoIP cache lock task failed")?
}

fn install_database(path: &Path, archive: &[u8]) -> Result<()> {
    ensure!(
        sha256(archive) == ARCHIVE_SHA256,
        "GeoIP archive checksum mismatch"
    );

    let mut database = Vec::new();
    GzDecoder::new(archive)
        .read_to_end(&mut database)
        .context("failed to decompress GeoIP country database")?;
    ensure!(
        sha256(&database) == DATABASE_SHA256,
        "GeoIP database checksum mismatch"
    );

    Reader::from_source(database.as_slice()).context("downloaded GeoIP database is invalid")?;

    let temporary = path.with_extension("mmdb.part");
    let result = (|| -> Result<()> {
        let mut file = File::create(&temporary)
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        file.write_all(&database)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temporary.display()))?;
        drop(file);

        fs::rename(&temporary, path)
            .with_context(|| format!("failed to publish {}", path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) fn location_without_database(value: &str) -> NodeLocation {
    match node_address(value) {
        NodeAddress::Private => NodeLocation::Private,
        NodeAddress::Public(_) | NodeAddress::Invalid => NodeLocation::Unavailable,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeAddress {
    Public(Ipv4Addr),
    Private,
    Invalid,
}

fn node_address(value: &str) -> NodeAddress {
    let Ok(ip) = value.parse::<Ipv4Addr>() else {
        return NodeAddress::Invalid;
    };
    let [first, second, third, _] = ip.octets();
    let non_public = first == 0
        || first == 10
        || first == 127
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 168)
        || (first == 192 && second == 0 && matches!(third, 0 | 2))
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || first >= 224;

    if non_public {
        NodeAddress::Private
    } else {
        NodeAddress::Public(ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_global_ipv4_addresses_are_geolocated() {
        assert_eq!(
            node_address("5.18.234.2"),
            NodeAddress::Public(Ipv4Addr::new(5, 18, 234, 2))
        );

        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.27.4",
            "169.254.1.1",
            "100.64.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "0.0.0.0",
            "not-an-ip",
        ] {
            let expected = if address == "not-an-ip" {
                NodeAddress::Invalid
            } else {
                NodeAddress::Private
            };
            assert_eq!(node_address(address), expected, "{address}");
        }
    }
}
