//! TON packages are append-only sequences of named binary files.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use tycho_types::models::BlockId;

/// Location of one file inside a TON archive or temporary package.
/// Offsets refer to the package payload, after its four-byte magic.
#[derive(Debug, Clone, Serialize)]
pub struct PackageEntry {
    pub name: String,
    pub offset: u64,
    pub size: u32,
    data_offset: u64,
}

impl PackageEntry {
    /// Returns the file category used by TON's `fileref` naming convention.
    pub fn kind(&self) -> &str {
        self.name
            .split_once('_')
            .map_or(self.name.as_str(), |x| x.0)
    }

    /// Reads the full ID from block, proof, or proof-link filenames.
    /// Other file categories remain available as opaque package entries.
    pub fn block_id(&self) -> Result<Option<BlockId>> {
        if !matches!(self.kind(), "block" | "proof" | "prooflink") {
            return Ok(None);
        }

        let (_, name) = self
            .name
            .split_once('_')
            .context("missing block filename")?;
        let (id, hashes) = name.split_once("):").context("invalid block filename")?;
        let id = id.strip_prefix('(').context("invalid block filename")?;

        format!("{}:{hashes}", id.replace(',', ":"))
            .parse()
            .map(Some)
            .with_context(|| format!("invalid block ID in {}", self.name))
    }
}

/// Streams a package without loading its payloads into memory.
/// Use a stopped node's database or an immutable snapshot: package appends and
/// index updates do not form a snapshot while a validator is running.
pub struct PackageReader {
    file: BufReader<File>,
    size: u64,
    next_offset: u64,
}

impl PackageReader {
    /// Opens an existing package and checks its format marker.
    pub fn open(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("cannot open package {}", path.display()))?;
        let size = file.metadata()?.len();
        let mut file = BufReader::new(file);
        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;
        ensure!(
            u32::from_le_bytes(magic) == 0xae8f_dd01,
            "invalid package magic"
        );

        Ok(Self {
            file,
            size,
            next_offset: 4,
        })
    }

    /// Reads the next entry header and skips its payload.
    /// A truncated final entry is an error, not a clean end of the package.
    pub fn next_entry(&mut self) -> Result<Option<PackageEntry>> {
        if self.next_offset == self.size {
            return Ok(None);
        }

        let offset = self.next_offset;
        self.file.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 8];
        self.file
            .read_exact(&mut header)
            .with_context(|| format!("truncated package entry at {offset}"))?;
        ensure!(
            header[..2] == [0x8b, 0x1e],
            "invalid package entry magic at {offset}"
        );

        let name_size = u16::from_le_bytes([header[2], header[3]]);
        let size = u32::from_le_bytes(header[4..].try_into()?);
        let data_offset = offset + 8 + u64::from(name_size);
        let end = data_offset + u64::from(size);
        ensure!(end <= self.size, "truncated package payload at {offset}");

        let mut name = vec![0; usize::from(name_size)];
        self.file.read_exact(&mut name)?;
        let name = String::from_utf8(name).context("package filename is not UTF-8")?;
        self.next_offset = end;

        Ok(Some(PackageEntry {
            name,
            offset: offset - 4,
            size,
            data_offset,
        }))
    }

    /// Loads a previously located entry from this package.
    /// The caller supplies a size limit before any payload allocation occurs.
    pub fn read_entry(&mut self, entry: &PackageEntry, max_bytes: usize) -> Result<Vec<u8>> {
        let size = usize::try_from(entry.size)?;
        ensure!(size <= max_bytes, "package entry exceeds {max_bytes} bytes");
        ensure!(
            entry.data_offset + u64::from(entry.size) <= self.size,
            "entry exceeds package size"
        );

        self.file.seek(SeekFrom::Start(entry.data_offset))?;
        let mut data = vec![0; size];
        self.file.read_exact(&mut data)?;

        Ok(data)
    }
}
