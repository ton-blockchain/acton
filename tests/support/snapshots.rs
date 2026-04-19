use crate::common::{assert_ui, strip_ansi};
use crate::regex;
use acton::build_info;
use snapbox::IntoData;
use snapbox::filter::Filter;
use std::mem;
use std::path::Path;

pub(crate) fn normalize_output(stdout: &str, project_path: &Path) -> String {
    normalize_output_internal(stdout, project_path, true, true)
}

#[allow(dead_code)]
pub(crate) fn normalize_output_keep_ansi(stdout: &str, project_path: &Path) -> String {
    normalize_output_internal(stdout, project_path, false, true)
}

#[allow(dead_code)]
pub(crate) fn normalize_output_preserve_escapes(stdout: &str, project_path: &Path) -> String {
    let content = strip_ansi(stdout);
    let mut value: serde_json::Value = serde_json::from_str(&content).unwrap_or_else(|err| {
        panic!("Expected valid JSON content for snapshot normalization: {err}");
    });

    let redactions = build_redactions(project_path);
    redact_json_value(&mut value, &redactions);

    normalize_up_snapshot_text(
        serde_json::to_string_pretty(&value)
            .expect("failed to serialize normalized JSON snapshot")
            .replace("\r\n", "\n"),
    )
}

fn normalize_output_internal(
    stdout: &str,
    project_path: &Path,
    strip: bool,
    use_path_filter: bool,
) -> String {
    let content = if strip {
        strip_ansi(stdout)
    } else {
        stdout.to_string()
    };
    let content = content.into_data();
    let content = if use_path_filter {
        snapbox::filter::FilterPaths.filter(content.into_data())
    } else {
        content.into_data()
    };
    let content = snapbox::filter::FilterNewlines.filter(content);
    let content = content.render().expect("came in as a String");

    let redactions = build_redactions(project_path);

    normalize_dynamic_output(redactions.redact(&content))
}

fn normalize_dynamic_output(content: String) -> String {
    normalize_dynamic_mutation_output(normalize_up_snapshot_text(content))
}

fn normalize_dynamic_mutation_output(content: String) -> String {
    regex!(r"(?m)^Session:\s+[0-9a-f]{16}$")
        .replace_all(&content, "Session:  [MUTATION_SESSION_ID]")
        .into_owned()
}

fn normalize_up_snapshot_text(content: String) -> String {
    let target_triple = build_info::TARGET_TRIPLE;
    let archive_name = format!("acton-{target_triple}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");

    content
        .replace(&checksum_name, "[ACTON_ARCHIVE_SHA256]")
        .replace(&archive_name, "[ACTON_ARCHIVE]")
        .replace(target_triple, "[TARGET_TRIPLE]")
}

fn build_redactions(project_path: &Path) -> snapbox::Redactions {
    let assert1 = assert_ui();
    let mut redactions = assert1.redactions().clone();

    let tmp_dir_raw = project_path.to_string_lossy().to_string();
    let tmp_dir_unix = if cfg!(windows) {
        tmp_dir_raw.replace('\\', "/")
    } else {
        tmp_dir_raw.clone()
    };
    let tmp_dir_raw_escaped = tmp_dir_raw.replace('\\', "\\\\");
    let tmp_dir_unix_escaped = tmp_dir_unix.replace('\\', "\\\\");

    let current_version = env!("CARGO_PKG_VERSION");

    redactions.insert("[ROOT]", tmp_dir_raw.clone()).unwrap();
    redactions.insert("[ROOT]", tmp_dir_unix.clone()).unwrap();
    redactions.insert("[ROOT]", tmp_dir_raw_escaped).unwrap();
    redactions.insert("[ROOT]", tmp_dir_unix_escaped).unwrap();
    redactions
        .insert("[ROOT]", "/private".to_owned() + tmp_dir_raw.as_str())
        .unwrap();
    redactions
        .insert("[ROOT]", "/private".to_owned() + tmp_dir_unix.as_str())
        .unwrap();
    redactions
        .insert(
            "[DATE]",
            regex!(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}[+-]\d{2}:\d{2}"),
        )
        .unwrap();
    redactions
        .insert("[DURATION]", regex!(r"duration='\d+'"))
        .unwrap();
    redactions
        .insert("[TIME]", regex!(r#"time="\d+\.\d+""#))
        .unwrap();
    redactions
        .insert("[BOC_HEX]", regex!("b5ee[\\d\\w]*"))
        .unwrap();
    redactions
        .insert("[WALLET_ADDRESS]", regex!("Wallet address is .*"))
        .unwrap();
    redactions
        .insert(
            "[WALLET_ADDRESS_BASE]",
            regex!(r"(?:EQ|UQ|kQ|0Q)[A-Za-z0-9_-]{46}"),
        )
        .unwrap();
    redactions
        .insert("[GLOBAL_WALLET_ADDRESS]", regex!("global-wallet .*"))
        .unwrap();
    redactions
        .insert("[LOCAL_WALLET_ADDRESS]", regex!("local-wallet .*"))
        .unwrap();
    redactions
        .insert("[WALLET_ADDRESS_TESTNET]", regex!("address-testnet = .*"))
        .unwrap();
    redactions
        .insert("[MNEMONIC]", regex!(r#"mnemonic = "[^"]*""#))
        .unwrap();
    redactions
        .insert(
            "[SECURITY_WARNING_MNEMONIC]",
            regex!(r"- The mnemonic is stored in plain text in .*"),
        )
        .unwrap();
    redactions
        .insert(
            "[SYMLINK_CREATED]",
            regex!(r"✓ Created symlink global.wallets.toml -> .*"),
        )
        .unwrap();
    redactions
        .insert(
            "[WALLET_CREATED_ADDED]",
            regex!(r"✓ Wallet successfully created and added to .*"),
        )
        .unwrap();
    redactions
        .insert("[DEPLOYED_AT]", regex!(r"Deployed at: .*"))
        .unwrap();
    redactions
        .insert(
            "[EXPLORER_TX_URL]",
            regex!(r"http://(?:localhost|127\.0\.0\.1):\d+/explorer/tx/[0-9a-fA-F]+"),
        )
        .unwrap();
    redactions
        .insert(
            "[LOCALHOST_URL]",
            regex!(r"http://(?:localhost|127\.0\.0\.1):\d+"),
        )
        .unwrap();
    redactions
        .insert(
            "[LOCALHOST_ADDR]",
            regex!(r"(?:localhost|127\.0\.0\.1):\d+"),
        )
        .unwrap();
    redactions
        .insert("[LAST_TOPUP_AT]", regex!(r"Last top-up: .*"))
        .unwrap();
    redactions
        .insert("[ACTON_VERSION]", format!("v{current_version}"))
        .unwrap();
    redactions
        .insert(
            "[ACTON_DOCS_URL]",
            "https://ton-blockchain.github.io/acton/docs",
        )
        .unwrap();
    redactions
        .insert("[OS_ERROR]", regex!(r"os error \d+"))
        .unwrap();

    redactions
}

#[allow(dead_code)]
fn redact_json_value(value: &mut serde_json::Value, redactions: &snapbox::Redactions) {
    match value {
        serde_json::Value::String(text) => {
            *text = redactions.redact(text);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item, redactions);
            }
        }
        serde_json::Value::Object(object) => {
            let old = mem::take(object);
            for (key, mut item) in old {
                let key = redactions.redact(&key);
                redact_json_value(&mut item, redactions);
                object.insert(key, item);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}
