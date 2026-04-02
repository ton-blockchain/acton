use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::ffi::{CStr, c_char};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeEmulatorVersion {
    #[serde(rename = "emulatorLibCommitHash")]
    pub ton_commit_hash: String,
    #[serde(rename = "emulatorLibCommitDate")]
    pub ton_commit_date: String,
}

pub fn native_emulator_version() -> Result<NativeEmulatorVersion> {
    // SAFETY: `emulator_version` is a pure native accessor that returns either
    // a valid null-terminated string allocated by the emulator or a null pointer.
    #[allow(unsafe_code)]
    let raw = unsafe { emulator_version() };
    if raw.is_null() {
        bail!("native emulator returned a null version pointer");
    }

    // SAFETY: `raw` was checked for null above and points to a valid
    // null-terminated string owned by the native library; `string_destroy`
    // is the matching deallocator exported by that library.
    #[allow(unsafe_code)]
    let json = unsafe {
        let json = CStr::from_ptr(raw).to_string_lossy().into_owned();
        string_destroy(raw);
        json
    };

    parse_native_emulator_version(&json)
}

fn parse_native_emulator_version(json: &str) -> Result<NativeEmulatorVersion> {
    serde_json::from_str(json).context("failed to parse native emulator version JSON")
}

#[allow(unsafe_code)]
unsafe extern "C" {
    fn emulator_version() -> *const c_char;
    fn string_destroy(str: *const c_char);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulator_version_json_deserializes() {
        let version = parse_native_emulator_version(
            r#"{
                "emulatorLibCommitHash":"f262bae",
                "emulatorLibCommitDate":"2026-03-25T00:00:00Z"
            }"#,
        )
        .expect("failed to parse emulator version JSON");

        assert_eq!(version.ton_commit_hash, "f262bae");
        assert_eq!(version.ton_commit_date, "2026-03-25T00:00:00Z");
    }
}
