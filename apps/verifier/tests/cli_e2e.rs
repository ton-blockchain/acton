mod support;

use std::{
    collections::BTreeMap,
    fs,
    io::{Seek, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::Arc,
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use verifier::{
    code_hash_policy::CodeHashPolicy,
    compilers::CompileGeneratedSource,
    config::Config,
    payment::{OnchainPaymentVerifier, PaymentLedger},
    registry::SourceVerificationRegistry,
    registry_index::{IndexedAbiContractsQuery, SqliteVerificationIndex, VerificationIndex},
    source_storage::{GitSourceStorage, SourceStorage},
    state::AppState,
};

const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MINIMAL_CONFIG: &str =
    "[server]\nread_only = true\n[source_repository]\npath = 'source-repo'\n";
const CONFIRMATION_PROMPT: &str = "Backend must be stopped. Type 'yes' to confirm: ";
const PUSH_DISABLED_WARNING: &str =
    "Warning: push_enabled = false; commits will remain local and will not be pushed.\n";

fn fixture(config: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temporary CLI directory");
    fs::write(dir.path().join("config.toml"), config).expect("CLI config");
    dir
}

fn cli(dir: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_verifier-cli"));
    command
        .current_dir(dir.path())
        .env_remove("VERIFIER_CONFIG")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(confirmation_input(b"yes\nyes\n"));
    command
}

fn confirmation_input(input: &[u8]) -> fs::File {
    let mut file = tempfile::tempfile().expect("confirmation input");
    file.write_all(input).expect("write confirmation");
    file.rewind().expect("rewind confirmation");
    file
}

fn assert_success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("Config: "), "{output:?}");
    assert!(stderr.contains("Checking repository: "), "{output:?}");
    assert!(stderr.contains("\nRemoval plan: "), "{output:?}");
    String::from_utf8(output.stdout).expect("UTF-8 CLI output")
}

fn assert_failure(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "{stderr}");
}

#[test]
fn removal_requires_read_only_but_dry_run_does_not() {
    for server in ["", "[server]\nread_only = false\n"] {
        let dir = fixture(&format!(
            "{server}[source_repository]\npath = 'source-repo'\n"
        ));
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let output = run_preserving(&dir, &repo, &["remove-registry", CODE_HASH]);
        assert_failure(&output, "requires server.read_only = true");
        assert!(!String::from_utf8_lossy(&output.stderr).contains(CONFIRMATION_PROMPT));
        let output = run_preserving_command(
            &dir,
            &repo,
            cli(&dir)
                .args(["remove-registry", "--dry-run", CODE_HASH])
                .stdin(Stdio::null()),
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains(CONFIRMATION_PROMPT));
        assert_success(output);
    }
}

#[test]
fn removal_requires_explicit_backend_shutdown_confirmation() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    for input in ["", "\n", "no\n", "y\n", "yes please\n"] {
        let output = run_preserving_command(
            &dir,
            &repo,
            cli(&dir)
                .args(["remove-registry", CODE_HASH])
                .stdin(confirmation_input(input.as_bytes())),
        );
        assert_failure(&output, CONFIRMATION_PROMPT);
        assert_failure(&output, "backend shutdown was not confirmed");
    }
}

#[test]
fn removal_requires_confirmation_after_inspecting_all_hashes() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let absent_hash = "bb".repeat(32);
    for answer in ["", "\n", "no\n", "y\n", "yes please\n", "no\nyes\n"] {
        let output = run_preserving_command(
            &dir,
            &repo,
            cli(&dir)
                .args(["remove-registry", CODE_HASH, &absent_hash, CODE_HASH])
                .stdin(confirmation_input(format!("yes\n{answer}").as_bytes())),
        );
        assert_failure(&output, "bundle removal was not confirmed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(
                "Removal plan: 1 found, 1 not found\nRemove 1 bundles? Type 'yes' to confirm: "
            ),
            "{stderr}"
        );
        assert!(!stderr.contains("[remove "), "{stderr}");
        assert!(!stderr.contains("Pushing "), "{stderr}");
    }
}

#[test]
fn invalid_utf8_in_piped_confirmation_never_confirms_removal() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    for input in [
        b"\xd0yes".as_slice(),
        b"\xd0yes\n",
        b"\xd0yes\n\n",
        b"\xd0yes\nno\nyes\n",
        b"\xd0yes\nyes\n",
        b"\xd0yes\n\xd1\n",
    ] {
        let output = run_preserving_command(
            &dir,
            &repo,
            cli(&dir)
                .args(["remove-registry", CODE_HASH])
                .stdin(confirmation_input(input)),
        );
        assert_failure(&output, "failed to read confirmation:");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Checking repository:"));
    }
}

#[test]
fn prepares_sorted_unique_hashes_from_hex_and_all_base64_formats() {
    let dir = fixture(MINIMAL_CONFIG);
    prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let lower_hash = "ab".repeat(32);
    let upper_hash = "fb".repeat(32);
    let output = cli(&dir)
        .args(["remove-registry", "--dry-run", "--"])
        .args([
            upper_hash.to_uppercase(),
            STANDARD.encode([0xfb; 32]),
            STANDARD_NO_PAD.encode([0xfb; 32]),
            URL_SAFE.encode([0xfb; 32]),
            URL_SAFE_NO_PAD.encode([0xfb; 32]),
            format!("  {}\n", lower_hash.to_uppercase()),
            lower_hash.clone(),
        ])
        .output()
        .expect("run CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Remote: origin\n"));
    assert!(stderr.contains("Storage root: sources\n"));
    assert!(stderr.contains("Branch: main\n"));
    let stdout = assert_success(output);

    assert!(stdout.contains(&format!("Code hashes: {lower_hash}, {upper_hash}\n")));
    assert!(stdout.contains("Dry run: registry removal plan"));
    assert!(stdout.contains(&format!("  {lower_hash}: not found\n")));
    assert!(stdout.contains(&format!("  {upper_hash}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 0\nNot found: 2\n"));
    assert!(!stdout.contains("Delete:"));
    assert!(!stdout.contains("Reason:"));
}

#[test]
fn uses_configured_repository_settings() {
    let dir = fixture(
        "[source_repository]\n\
         path = 'custom repo'\n\
         storage_root = 'verified/contracts'\n\
         remote = 'upstream'\n\
         branch = 'registry'\n\
         author_name = 'Registry Operator'\n\
         author_email = 'operator@example.invalid'\n\
         commit_enabled = false\n\
         push_enabled = false\n",
    );
    prepare_repository(
        &dir,
        "custom repo",
        "verified/contracts",
        "upstream",
        "registry",
    );
    let output = cli(&dir)
        .args(["remove-registry", "--dry-run", CODE_HASH])
        .output()
        .expect("run CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "Repository: custom repo\n",
        "Storage root: verified/contracts\n",
        "Remote: upstream\n",
        "Branch: registry\n",
    ] {
        assert!(stderr.contains(expected), "{stderr}");
    }
    let stdout = assert_success(output);
    for expected in [
        &format!("    Path: {}\n", bundle_path("verified/contracts")),
        &format!("  {CODE_HASH}: found\n"),
    ] {
        assert!(stdout.contains(expected), "{stdout}");
    }
}

#[test]
fn config_path_uses_flag_then_environment_then_default() {
    let dir = fixture(MINIMAL_CONFIG);
    prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    for (filename, repository) in [("env.toml", "env-repo"), ("flag.toml", "flag-repo")] {
        fs::write(
            dir.path().join(filename),
            format!("[source_repository]\npath = '{repository}'\n"),
        )
        .expect("alternative config");
        prepare_repository(&dir, repository, "sources", "origin", "main");
    }

    for (environment, explicit, expected_repository) in [
        (None, None, "source-repo"),
        (Some("env.toml"), None, "env-repo"),
        (Some("missing-env.toml"), Some("flag.toml"), "flag-repo"),
    ] {
        let mut command = cli(&dir);
        command.args(["remove-registry", "--dry-run", CODE_HASH]);
        if let Some(config_path) = environment {
            command.env("VERIFIER_CONFIG", config_path);
        }
        if let Some(config_path) = explicit {
            command.args(["--config", config_path]);
        }
        let output = command.output().expect("run CLI");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("Repository: {expected_repository}\n")),
            "{stderr}"
        );
        assert_success(output);
    }
}

#[test]
fn rejects_missing_or_empty_source_repository_path() {
    for config in ["", "[source_repository]\npath = ''\n"] {
        let dir = fixture(config);
        let output = cli(&dir)
            .env("PATH", "")
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        assert_failure(
            &output,
            "source_repository.path must be set to a non-empty path in config config.toml",
        );
    }
}

#[test]
fn rejects_invalid_hashes_without_partial_output() {
    let dir = fixture(MINIMAL_CONFIG);
    for invalid in [
        String::new(),
        "  ".to_owned(),
        "not-a-hash".to_owned(),
        "g".repeat(64),
        "é".repeat(32),
        "a".repeat(63),
        "a".repeat(65),
        STANDARD.encode([0; 31]),
        STANDARD.encode([0; 33]),
    ] {
        for dry_run in [false, true] {
            let mut command = cli(&dir);
            // Invalid input must fail before Git or any other executable runs.
            command.env("PATH", "");
            command.arg("remove-registry");
            if dry_run {
                command.arg("--dry-run");
            }
            let output = command
                .args([CODE_HASH, &invalid, CODE_HASH])
                .output()
                .expect("run CLI");
            assert_failure(&output, &format!("invalid code hash {invalid:?}"));
            assert_failure(
                &output,
                "expected a 256-bit code hash in hexadecimal or base64",
            );
        }
    }
}

#[test]
fn reports_missing_config_with_its_path() {
    let dir = fixture(MINIMAL_CONFIG);
    let output = cli(&dir)
        .args([
            "remove-registry",
            "--dry-run",
            "--config",
            "missing.toml",
            CODE_HASH,
        ])
        .output()
        .expect("run CLI");
    assert_failure(&output, "failed to read config missing.toml");
}

#[test]
fn reports_config_parse_and_validation_errors() {
    for (config, expected) in [
        ("[source_repository", "failed to parse config config.toml"),
        (
            "[compiler]\nmax_concurrent_compilations = 0\n",
            "compiler max_concurrent_compilations must be -1 or a positive integer, got 0",
        ),
    ] {
        let dir = fixture(config);
        let output = cli(&dir)
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        assert_failure(&output, expected);
    }
}

#[test]
fn absent_hashes_succeed_without_commit_or_push() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let hash = "bb".repeat(32);
    let output = run_preserving_command(
        &dir,
        &repo,
        cli(&dir)
            .args(["remove-registry", &hash])
            .stdin(confirmation_input(b"yes\n")),
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Remove 0 bundles?"));
    let stdout = assert_success(output);
    assert!(stdout.contains(&format!("  {hash}: not found\n")));
    assert!(stdout.contains("Removed bundles: 0\nNot found: 1\n"));
    assert!(stdout.contains("No bundles found; nothing to remove.\n"));
    assert!(!stdout.contains("Commit:"));
    assert!(!stdout.contains("Push:"));
}

#[test]
fn checks_preserve_repository_and_database_files() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let files = [
        ("verifier-index.sqlite3", "registry sentinel"),
        ("verifier-payments.sqlite3", "payment ledger sentinel"),
    ];
    for (filename, contents) in files {
        fs::write(dir.path().join(filename), contents).expect("sentinel file");
    }
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        stderr.contains(&format!("Local HEAD: {head}\n")),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!("Remote HEAD: {head}\n")),
        "{stderr}"
    );
    assert_success(output);

    for (filename, contents) in files {
        assert_eq!(
            fs::read_to_string(dir.path().join(filename)).expect("preserved file"),
            contents,
            "{filename}"
        );
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("config.toml")).expect("preserved config"),
        MINIMAL_CONFIG
    );
}

#[test]
fn rejects_missing_checkout_without_creating_it() {
    let dir = fixture(MINIMAL_CONFIG);
    let before = directory_entries(dir.path());
    for dry_run in [false, true] {
        let mut command = cli(&dir);
        command.args(["remove-registry", CODE_HASH]);
        if dry_run {
            command.arg("--dry-run");
        }
        assert_failure(&command.output().expect("run CLI"), "path does not exist");
        assert_eq!(directory_entries(dir.path()), before);
    }
}

#[test]
fn rejects_non_git_bare_and_unborn_repositories() {
    for state in ["non-git", "bare", "unborn"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = dir.path().join("source-repo");
        fs::create_dir(&repo).expect("repository directory");
        match state {
            "bare" => {
                git(&repo, &["init", "--bare"]);
            }
            "unborn" => {
                git(&repo, &["init", "-b", "main"]);
            }
            _ => {}
        }
        let before = directory_entries(dir.path());
        let output = cli(&dir)
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        let expected = match state {
            "bare" => "path is not inside a git work tree",
            "unborn" => "git rev-parse --verify HEAD",
            _ => "not a git repository",
        };
        assert_failure(&output, expected);
        assert_eq!(directory_entries(dir.path()), before, "{state}");
    }
}

#[test]
fn rejects_checkout_subdirectory() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    fs::write(
        dir.path().join("config.toml"),
        "[source_repository]\npath = 'source-repo/sources'\n",
    )
    .expect("config");
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    assert_failure(&output, "must point to the checkout root");
}

#[test]
fn rejects_detached_head_even_with_configured_branch() {
    for branch in ["", "branch = 'main'\n"] {
        let dir = fixture(&format!("{MINIMAL_CONFIG}{branch}"));
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        git(&repo, &["checkout", "--detach"]);
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "has detached HEAD; check out a named branch");
    }
}

#[test]
fn rejects_unexpected_branch() {
    let dir = fixture(&format!("{MINIMAL_CONFIG}branch = 'registry'\n"));
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    assert_failure(&output, "branch mismatch: expected registry, current main");
}

#[test]
fn rejects_dirty_checkout_without_startup_recovery() {
    for state in ["unstaged", "staged", "untracked", "deleted", "mixed"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let source = repo.join(bundle_path("sources")).join("source.tolk");
        match state {
            "untracked" => fs::write(source.with_file_name("pending.tolk"), "untracked\n"),
            "deleted" => fs::remove_file(&source),
            _ => fs::write(&source, "modified\n"),
        }
        .expect("dirty checkout");
        if matches!(state, "staged" | "mixed") {
            git(&repo, &["add", "--all"]);
        }
        if state == "mixed" {
            fs::write(&source, "unstaged after staging\n").expect("unstaged content");
            fs::write(source.with_file_name("pending.tolk"), "untracked\n")
                .expect("untracked content");
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "has uncommitted changes");
    }
}

#[test]
fn uses_shared_source_repository_validation() {
    for state in ["invalid-root", "missing-rule", "overridden-rule"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let attributes = match state {
            "overridden-rule" => "sources/** -text\nsources/** text\n",
            _ => "sources/** text\n",
        };
        if state == "invalid-root" {
            let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
            git(&repo, &["checkout", "-b", "invalid-root", &root]);
        }
        fs::write(repo.join(".gitattributes"), attributes).expect("attributes");
        git(&repo, &["add", "--all"]);
        if state == "invalid-root" {
            git(
                &repo,
                &["commit", "--amend", "-m", "Invalid initial attributes"],
            );
        } else {
            git(&repo, &["commit", "-m", "Invalid current attributes"]);
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "is not prepared");
        assert_failure(
            &output,
            match state {
                "invalid-root" => "root `.gitattributes` must contain exactly",
                "missing-rule" => "current `.gitattributes` must contain",
                _ => "is not effective",
            },
        );
    }
}

#[test]
fn allows_missing_storage_and_bundle_directories() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&repo, &["checkout", "-b", "empty", &root]);
    git(&repo, &["push", "origin", "HEAD:empty"]);
    assert!(!repo.join("sources").exists());
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stdout = assert_success(output);
    assert!(stdout.contains(&format!("  {CODE_HASH}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 0\nNot found: 1\n"));
    assert!(!repo.join("sources").exists());
}

#[test]
fn rejects_local_head_behind_ahead_or_diverged_from_remote() {
    for state in ["behind", "ahead", "diverged"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let remote = dir.path().join("source-repo.git");
        let original = git(&repo, &["rev-parse", "HEAD"]);
        if matches!(state, "behind" | "diverged") {
            // Keep the original checkout's remote-tracking ref stale.
            git(
                dir.path(),
                &[
                    "clone",
                    "--branch",
                    "main",
                    "source-repo.git",
                    "other-writer",
                ],
            );
            let writer = dir.path().join("other-writer");
            git(
                &writer,
                &["commit", "--allow-empty", "-m", "Remote advancement"],
            );
            git(&writer, &["push", "origin", "HEAD:main"]);
        }
        if matches!(state, "ahead" | "diverged") {
            git(
                &repo,
                &["commit", "--allow-empty", "-m", "Local advancement"],
            );
        }
        assert_eq!(
            git(&repo, &["rev-parse", "refs/remotes/origin/main"]),
            original
        );
        let local_head = git(&repo, &["rev-parse", "HEAD"]);
        let remote_head = git(&remote, &["rev-parse", "refs/heads/main"]);
        assert_ne!(local_head, remote_head);
        for dry_run in [false, true] {
            let args = if dry_run {
                vec!["remove-registry", "--dry-run", CODE_HASH]
            } else {
                vec!["remove-registry", CODE_HASH]
            };
            let output = run_preserving(&dir, &repo, &args);
            assert_failure(&output, "HEAD does not match origin/main");
            assert_failure(&output, &format!("local HEAD={local_head}"));
            assert_failure(&output, &format!("remote HEAD={remote_head}"));
        }
    }
}

#[test]
fn accepts_matching_remote_even_with_stale_or_missing_tracking_reference() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&repo, &["update-ref", "refs/remotes/origin/main", &root]);
    for missing in [false, true] {
        if missing {
            git(&repo, &["update-ref", "-d", "refs/remotes/origin/main"]);
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_success(output);
    }
}

#[test]
fn rejects_unavailable_remote_and_missing_branch() {
    for state in ["unavailable", "missing-branch", "unconfigured"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let remote = dir.path().join("source-repo.git");
        match state {
            "missing-branch" => {
                git(&remote, &["update-ref", "-d", "refs/heads/main"]);
            }
            "unconfigured" => {
                git(&repo, &["remote", "remove", "origin"]);
            }
            _ => {
                git(
                    &repo,
                    &["remote", "set-url", "origin", "../unavailable.git"],
                );
            }
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "git command failed");
        assert_failure(
            &output,
            "git ls-remote --exit-code --refs -- origin refs/heads/main",
        );
        assert!(!dir.path().join("unavailable.git").exists());
    }
}

#[test]
fn checks_the_configured_branch_instead_of_remote_head() {
    let dir = fixture(&format!("{MINIMAL_CONFIG}branch = 'registry/production'\n"));
    let repo = prepare_repository(
        &dir,
        "source-repo",
        "sources",
        "origin",
        "registry/production",
    );
    let remote = dir.path().join("source-repo.git");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&remote, &["update-ref", "refs/heads/unrelated", &root]);
    git(&remote, &["symbolic-ref", "HEAD", "refs/heads/unrelated"]);
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Branch: registry/production\n"), "{stderr}");
    assert_success(output);
}

#[test]
fn dry_run_prints_a_stable_complete_plan_for_present_and_absent_bundles() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let first = bundle_path("sources");
    let second_hash = "bb".repeat(32);
    let second = bundle_path_for("sources", &second_hash);
    let absent_hash = "ab".repeat(32);
    let absent = bundle_path_for("sources", &absent_hash);
    for (relative, contents) in [
        (format!("{first}/abi.json"), "{}"),
        (format!("{first}/src/lib.tolk"), "// nested source"),
        // No manifest: the directory layout alone identifies a bundle.
        (format!("{second}/source.tolk"), "// second source"),
        (
            format!("{first}-sibling/source.tolk"),
            "// outside selected directory",
        ),
        (
            format!("{absent}-sibling/source.tolk"),
            "// not the absent bundle",
        ),
        (
            format!(
                "{}/source.tolk",
                bundle_path_for("sources", &"cc".repeat(32))
            ),
            "// unselected bundle",
        ),
    ] {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent directory")).expect("source directory");
        fs::write(path, contents).expect("source file");
    }
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add plan fixtures"]);
    git(&repo, &["push", "origin", "HEAD:main"]);
    let expected = format!(
        "Dry run: registry removal plan\n\
         Code hashes: {CODE_HASH}, {absent_hash}, {second_hash}\n\
         Bundles:\n  {CODE_HASH}: found\n    Path: {first}\n  {absent_hash}: not found\n    Path: {absent}\n  {second_hash}: found\n    Path: {second}\n\
         Bundles to remove: 2\n\
         Not found: 1\n"
    );
    for hashes in [
        vec![second_hash.as_str(), absent_hash.as_str(), CODE_HASH],
        vec![
            CODE_HASH,
            absent_hash.as_str(),
            second_hash.as_str(),
            CODE_HASH,
        ],
    ] {
        let mut args = vec!["remove-registry", "--dry-run"];
        args.extend(hashes);
        let stdout = assert_success(run_preserving(&dir, &repo, &args));
        assert_eq!(stdout, expected);
    }
}

#[test]
fn removes_entire_selected_directories_and_preserves_other_files() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let ignored_hash = "bb".repeat(32);
    let ignored_bundle = bundle_path_for("sources", &ignored_hash);
    let absent_hash = "cc".repeat(32);
    let selected = bundle_path("sources");
    fs::write(
        repo.join(".git/info/exclude"),
        format!("*.cache\n/{ignored_bundle}/manifest.json\n"),
    )
    .expect("ignore rules");
    let removed = [
        format!("{selected}/pending.cache"),
        format!("{selected}/nested/pending.cache"),
        format!("{ignored_bundle}/manifest.json"),
    ];
    let preserved = [
        "local.cache".to_owned(),
        format!("{selected}-sibling/pending.cache"),
    ];
    for relative in removed.iter().chain(&preserved) {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent directory")).expect("ignored directory");
        fs::write(path, "local contents").expect("ignored file");
    }
    let stdout = assert_success(run_preserving(
        &dir,
        &repo,
        &[
            "remove-registry",
            "--dry-run",
            CODE_HASH,
            &ignored_hash,
            &absent_hash,
        ],
    ));
    assert!(stdout.contains(&format!("  {ignored_hash}: found\n")));
    assert!(stdout.contains(&format!("  {absent_hash}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 2\nNot found: 1\n"));
    assert!(!stdout.contains(".cache"));
    let stdout = assert_success(
        cli(&dir)
            .args(["remove-registry", CODE_HASH, &ignored_hash, &absent_hash])
            .output()
            .expect("run CLI"),
    );
    assert!(stdout.contains("Removed bundles: 2\nNot found: 1\n"));
    assert!(!repo.join(&selected).exists());
    assert!(!repo.join(ignored_bundle).exists());
    for relative in &preserved {
        assert_eq!(
            fs::read_to_string(repo.join(relative)).expect("ignored contents"),
            "local contents"
        );
    }
    assert_eq!(git(&repo, &["ls-files"]), ".gitattributes");
    assert_eq!(
        git(&repo, &["diff", "HEAD^", "HEAD", "--name-status"]),
        format!("D\t{selected}/manifest.json\nD\t{selected}/source.tolk")
    );
}

#[test]
fn removes_directories_without_tracked_files_without_staging_changes() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let ignored_hash = "bb".repeat(32);
    let empty_hash = "cc".repeat(32);
    let ignored_bundle = repo.join(bundle_path_for("sources", &ignored_hash));
    let empty_bundle = repo.join(bundle_path_for("sources", &empty_hash));
    fs::write(repo.join(".git/info/exclude"), "*.cache\n").expect("ignore rules");
    fs::create_dir_all(&ignored_bundle).expect("ignored bundle directory");
    fs::create_dir_all(&empty_bundle).expect("empty bundle directory");
    fs::write(ignored_bundle.join("pending.cache"), "local contents").expect("ignored file");
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let tracked_bundle = repo.join(bundle_path("sources"));
    let before = directory_entries(&tracked_bundle);
    let stdout = assert_success(run_preserving(
        &dir,
        &repo,
        &["remove-registry", "--dry-run", &ignored_hash, &empty_hash],
    ));
    assert!(stdout.contains("Bundles to remove: 2\nNot found: 0\n"));
    let stdout = assert_success(
        cli(&dir)
            .args(["remove-registry", &ignored_hash, &empty_hash])
            .output()
            .expect("run CLI"),
    );
    assert!(stdout.contains("Removed bundles: 2\nNot found: 0\n"));
    assert!(!stdout.contains("Deletions staged in Git."));
    assert!(!ignored_bundle.exists());
    assert!(!empty_bundle.exists());
    assert_eq!(directory_entries(&tracked_bundle), before);
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        head
    );
}

#[test]
fn directory_removal_failure_preserves_earlier_commits() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let invalid_hash = "bb".repeat(32);
    let invalid_bundle = bundle_path_for("sources", &invalid_hash);
    fs::write(
        repo.join(".git/info/exclude"),
        format!("/{invalid_bundle}\n"),
    )
    .expect("ignore rules");
    fs::create_dir_all(repo.join("sources/bb")).expect("shard directory");
    fs::write(repo.join(&invalid_bundle), "not a directory").expect("invalid bundle path");
    let tracked_bundle = repo.join(bundle_path("sources"));
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let output = cli(&dir)
        .args(["remove-registry", CODE_HASH, &invalid_hash])
        .output()
        .expect("run CLI");
    assert_failure(&output, "failed to remove directory");
    assert_failure(&output, "earlier local commits are preserved");
    assert_failure(&output, "push was not attempted");
    assert!(!tracked_bundle.exists());
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), head);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        head
    );
    assert_eq!(
        fs::read_to_string(repo.join(invalid_bundle)).expect("invalid bundle path"),
        "not a directory"
    );
}

#[test]
fn plans_and_removes_literal_paths_with_git_pathspec_characters() {
    for (root, other_root) in [("sources*", "sources-other"), (":(glob)sources", "sources")] {
        let dir = fixture(&format!("{MINIMAL_CONFIG}storage_root = '{root}'\n"));
        let repo = prepare_repository(&dir, "source-repo", root, "origin", "main");
        let selected = bundle_path(root);
        let other = bundle_path(other_root);
        fs::create_dir_all(repo.join(&other)).expect("other storage directory");
        fs::write(
            repo.join(format!("{other}/outside.tolk")),
            "// not selected",
        )
        .expect("other source");
        for filename in ["contract[1]*?.tolk", "source with spaces.tolk "] {
            fs::write(repo.join(&selected).join(filename), "// selected").expect("source filename");
        }
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "-m", "Add literal path fixtures"]);
        git(&repo, &["push", "origin", "HEAD:main"]);
        let stdout = assert_success(run_preserving(
            &dir,
            &repo,
            &["remove-registry", "--dry-run", CODE_HASH],
        ));
        assert!(stdout.contains(&format!("  {CODE_HASH}: found\n    Path: {selected}\n")));
        assert_success(
            cli(&dir)
                .args(["remove-registry", CODE_HASH])
                .output()
                .expect("run CLI"),
        );
        assert!(!repo.join(selected).exists());
        assert_eq!(
            fs::read_to_string(repo.join(format!("{other}/outside.tolk")))
                .expect("unselected source"),
            "// not selected"
        );
        assert_eq!(git(&repo, &["diff", "--name-only"]), "");
    }
}

#[test]
fn commits_each_bundle_and_pushes_without_changing_unrelated_files() {
    let dir = fixture(
        "[server]\nread_only = true\n[source_repository]\n\
         path = 'source-repo'\n\
         storage_root = 'verified/contracts'\n\
         remote = 'upstream'\n\
         branch = 'registry'\n\
         commit_enabled = false\n\
         push_enabled = true\n\
         author_name = 'Cleanup Bot'\n\
         author_email = 'cleanup@example.invalid'\n",
    );
    let root = "verified/contracts";
    let repo = prepare_repository(&dir, "source-repo", root, "upstream", "registry");
    let remote = dir.path().join("source-repo.git");
    let first = bundle_path(root);
    let second_hash = "bb".repeat(32);
    let second = bundle_path_for(root, &second_hash);
    let absent_hash = "ab".repeat(32);
    let absent = bundle_path_for(root, &absent_hash);
    let remaining_hash = "cc".repeat(32);
    let remaining = bundle_path_for(root, &remaining_hash);
    for (relative, contents) in [
        (format!("{first}/abi.json"), "{}"),
        (format!("{first}/src/lib.tolk"), "// nested source"),
        (format!("{second}/source.tolk"), "// no manifest"),
        (format!("{first}-sibling/source.tolk"), "// sibling"),
        (format!("{absent}-sibling/source.tolk"), "// absent sibling"),
        (format!("{remaining}/source.tolk"), "// remaining bundle"),
    ] {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("bundle directory");
        fs::write(path, contents).expect("source file");
    }
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add removal fixtures"]);
    git(&repo, &["push", "upstream", "HEAD:registry"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let config = fs::read(dir.path().join("config.toml")).expect("config bytes");
    for filename in ["registry.sqlite", "payments.sqlite", "blocklist.txt"] {
        fs::write(dir.path().join(filename), "unrelated contents").expect("unrelated file");
    }

    let stdout = assert_success(
        cli(&dir)
            .args([
                "remove-registry",
                &second_hash,
                &absent_hash,
                CODE_HASH,
                CODE_HASH,
            ])
            .output()
            .expect("run CLI"),
    );
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let first_commit = git(&repo, &["rev-parse", "HEAD^"]);
    assert_eq!(git(&repo, &["rev-parse", "HEAD~2"]), original);
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/registry"]), head);
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
    assert_eq!(
        git(
            &repo,
            &["diff", "--no-renames", "--name-status", &original, &head]
        ),
        format!(
            "D\t{first}/abi.json\nD\t{first}/manifest.json\nD\t{first}/source.tolk\nD\t{first}/src/lib.tolk\nD\t{second}/source.tolk"
        )
    );
    for (commit, hash, expected_files, manifest_hash) in [
        (
            &first_commit,
            CODE_HASH,
            format!(
                "D\t{first}/abi.json\nD\t{first}/manifest.json\nD\t{first}/source.tolk\nD\t{first}/src/lib.tolk"
            ),
            hex::encode(Sha256::digest(b"broken manifest\n")),
        ),
        (
            &head,
            second_hash.as_str(),
            format!("D\t{second}/source.tolk"),
            "unavailable".to_owned(),
        ),
    ] {
        assert_eq!(
            git(
                &repo,
                &["diff-tree", "--no-commit-id", "--name-status", "-r", commit]
            ),
            expected_files
        );
        assert_eq!(
            git(&repo, &["show", "-s", "--format=%B", commit]),
            format!(
                "Revoke verification for code hash {hash}\n\ncode_hash: {hash}\nsource_bundle_hash: unavailable\nmanifest_hash: {manifest_hash}"
            )
        );
        assert_eq!(
            git(
                &repo,
                &["show", "-s", "--format=%an <%ae>%n%cn <%ce>", commit]
            ),
            "Cleanup Bot <cleanup@example.invalid>\nCleanup Bot <cleanup@example.invalid>"
        );
        assert!(stdout.contains(&format!("    Commit: {commit}\n")));
    }
    assert!(!repo.join(&first).exists());
    assert!(!repo.join(second).exists());
    for (relative, expected) in [
        (format!("{remaining}/source.tolk"), "// remaining bundle"),
        (format!("{first}-sibling/source.tolk"), "// sibling"),
        (format!("{absent}-sibling/source.tolk"), "// absent sibling"),
    ] {
        assert_eq!(
            fs::read_to_string(repo.join(relative)).expect("unselected source"),
            expected
        );
    }
    assert!(stdout.contains(&format!("  {absent_hash}: not found\n")));
    assert!(stdout.contains("Removed bundles: 2\nNot found: 1\n"));
    assert!(stdout.contains("Push: upstream/registry\n"));
    for removed_field in ["Author:", "source_bundle_hash:", "manifest_hash:"] {
        assert!(!stdout.contains(removed_field), "{stdout}");
    }
    assert_eq!(
        fs::read(dir.path().join("config.toml")).expect("config bytes"),
        config
    );
    for filename in ["registry.sqlite", "payments.sqlite", "blocklist.txt"] {
        assert_eq!(
            fs::read_to_string(dir.path().join(filename)).expect("unrelated file"),
            "unrelated contents"
        );
    }
    let stdout = assert_success(run_preserving(&dir, &repo, &["remove-registry", CODE_HASH]));
    assert!(stdout.contains(&format!("{CODE_HASH}: not found")));
}

#[test]
fn disabled_push_keeps_commits_locally_without_contacting_a_remote() {
    for commit_enabled in [true, false] {
        let dir = fixture(&format!(
            "{MINIMAL_CONFIG}commit_enabled = {commit_enabled}\npush_enabled = false\n"
        ));
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let second_hash = "bb".repeat(32);
        let third_hash = "cc".repeat(32);
        let first = bundle_path("sources");
        let second = bundle_path_for("sources", &second_hash);
        let third = bundle_path_for("sources", &third_hash);
        for path in [&second, &third] {
            fs::create_dir_all(repo.join(path)).expect("bundle directory");
            fs::write(repo.join(path).join("source.tolk"), "// source").expect("source file");
        }
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "-m", "Add local removal fixtures"]);
        git(&repo, &["push", "origin", "HEAD:main"]);
        let original = git(&repo, &["rev-parse", "HEAD"]);
        // Any remote lookup or push would fail; offline removal must still work.
        git(&repo, &["remote", "remove", "origin"]);

        let output = run_preserving(
            &dir,
            &repo,
            &["remove-registry", "--dry-run", CODE_HASH, &second_hash],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(PUSH_DISABLED_WARNING));
        assert!(stderr.contains("Remote HEAD: not checked (push_enabled = false)\n"));
        assert!(stderr.contains("Push enabled: false\n"));
        let stdout = assert_success(output);
        assert!(stdout.contains("Bundles to remove: 2\n"));

        let output = cli(&dir)
            .args(["remove-registry", CODE_HASH, &second_hash])
            .output()
            .expect("run CLI");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("{PUSH_DISABLED_WARNING}{CONFIRMATION_PROMPT}")));
        let stdout = assert_success(output);
        assert!(!repo.join(&first).exists());
        assert!(!repo.join(&second).exists());
        assert!(repo.join(&third).exists());
        assert_eq!(git(&repo, &["diff", "--cached", "--name-status"]), "");
        assert!(stdout.contains("Removed bundles: 2\n"));
        assert!(!stdout.contains("Push: "));
        let expected_deletions =
            format!("D\t{first}/manifest.json\nD\t{first}/source.tolk\nD\t{second}/source.tolk");
        assert_eq!(git(&repo, &["rev-parse", "HEAD~2"]), original);
        assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
        assert_eq!(
            git(&repo, &["diff", "--name-status", &original, "HEAD"]),
            expected_deletions
        );
        assert_eq!(stdout.matches("    Commit: ").count(), 2);
        assert!(stdout.contains("Push skipped: push_enabled = false; commits kept locally."));
        // A later invocation can build on the unpublished local commits.
        assert_success(
            cli(&dir)
                .args(["remove-registry", &third_hash])
                .output()
                .expect("run CLI again"),
        );
        assert_eq!(git(&repo, &["rev-parse", "HEAD~3"]), original);
        assert!(!repo.join(&third).exists());
        assert_eq!(
            git(
                &dir.path().join("source-repo.git"),
                &["rev-parse", "refs/heads/main"]
            ),
            original
        );
    }
}

#[test]
fn local_removal_still_checks_checkout_and_branch() {
    let dir = fixture(&format!(
        "{MINIMAL_CONFIG}branch = 'main'\npush_enabled = false\n"
    ));
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    git(&repo, &["remote", "remove", "origin"]);
    git(&repo, &["checkout", "-b", "other"]);
    assert_failure(
        &run_preserving(&dir, &repo, &["remove-registry", CODE_HASH]),
        "branch mismatch: expected main, current other",
    );
    git(&repo, &["checkout", "main"]);
    fs::write(
        repo.join(bundle_path("sources")).join("source.tolk"),
        "// changed",
    )
    .expect("dirty source");
    assert_failure(
        &run_preserving(&dir, &repo, &["remove-registry", CODE_HASH]),
        "uncommitted",
    );
}

#[test]
fn commits_and_pushes_removal_of_the_last_bundle() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    assert_success(
        cli(&dir)
            .args(["remove-registry", CODE_HASH])
            .output()
            .expect("run CLI"),
    );
    assert_eq!(git(&repo, &["ls-files"]), ".gitattributes");
    assert!(!repo.join(bundle_path("sources")).exists());
    let head = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        head
    );
}

#[cfg(unix)]
#[test]
fn partial_removal_and_unexpected_staged_changes_restore_only_planned_paths() {
    for failure in ["remove", "staged-diff"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let original = git(&repo, &["rev-parse", "HEAD"]);
        let bundle = repo.join(bundle_path("sources"));
        let before = directory_entries(&bundle);
        let output = cli_with_git_script(
            &dir,
            r#"
if [ "$1" = --literal-pathspecs ] && [ "$2" = rm ]; then
    if [ "$CLI_TEST_FAILURE" = remove ]; then
        "$CLI_TEST_REAL_GIT" --literal-pathspecs rm -- "$CLI_TEST_FIRST_FILE"
    else
        "$CLI_TEST_REAL_GIT" "$@"
    fi
    printf 'unrelated staged contents\n' > unrelated.txt
    "$CLI_TEST_REAL_GIT" add -- unrelated.txt
    printf 'unrelated working contents\n' >> unrelated.txt
    if [ "$CLI_TEST_FAILURE" = remove ]; then
        echo 'injected removal failure' >&2
        exit 1
    fi
    exit 0
fi
"#,
        )
        .env("CLI_TEST_FAILURE", failure)
        .env(
            "CLI_TEST_FIRST_FILE",
            format!("{}/source.tolk", bundle_path("sources")),
        )
        .args(["remove-registry", CODE_HASH])
        .output()
        .expect("run CLI");
        assert_failure(
            &output,
            if failure == "remove" {
                "injected removal failure"
            } else {
                "staged diff does not match the removal plan"
            },
        );
        assert_failure(
            &output,
            "planned tracked files and index entries were restored",
        );
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
        assert_eq!(
            git(
                &dir.path().join("source-repo.git"),
                &["rev-parse", "refs/heads/main"]
            ),
            original
        );
        assert_eq!(directory_entries(&bundle), before);
        assert_eq!(
            git(&repo, &["diff", "--cached", "--name-status"]),
            "A\tunrelated.txt"
        );
        assert_eq!(
            git(&repo, &["show", ":unrelated.txt"]),
            "unrelated staged contents"
        );
        assert_eq!(
            fs::read_to_string(repo.join("unrelated.txt")).expect("unrelated worktree file"),
            "unrelated staged contents\nunrelated working contents\n"
        );
    }
}

#[cfg(unix)]
#[test]
fn recovery_failure_reports_both_errors_and_leaves_head_unchanged() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let output = cli_with_git_script(
        &dir,
        r#"
if [ "$1" = --literal-pathspecs ]; then
    case "$2" in
        rm) "$CLI_TEST_REAL_GIT" "$@"; echo 'injected removal failure' >&2; exit 1 ;;
        restore) echo 'injected restore failure' >&2; exit 1 ;;
    esac
fi
"#,
    )
    .args(["remove-registry", CODE_HASH])
    .output()
    .expect("run CLI");
    assert_failure(&output, "injected removal failure");
    assert_failure(&output, "injected restore failure");
    assert_failure(&output, "inspect git status before retrying");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        original
    );
    assert!(!git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn backend_restart_rebuilds_registry_after_cli_removal_without_changing_payments() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    git(&repo, &["rm", "-r", "--", "sources"]);
    git(&repo, &["commit", "-m", "Remove placeholder bundle"]);
    git(&repo, &["push", "origin", "HEAD:main"]);
    let index_path = dir.path().join("registry.sqlite");
    let ledger_path = dir.path().join("payments.sqlite");
    let blocklist_path = dir.path().join("blocked-code-hashes.txt");
    fs::write(&blocklist_path, CODE_HASH).expect("blocklist");
    let config_bytes = format!(
        "[server]\nread_only = true\nblocked_code_hashes_path = '{}'\n\
         [source_repository]\npath = '{}'\nbranch = 'main'\ncommit_enabled = true\npush_enabled = true\n\
         [registry_index]\npath = '{}'\n\
         [payment]\nledger_path = '{}'\n\
         [compiler]\nnode_bin = '/unavailable-node'\nworker_path = '/unavailable-worker'\n",
        blocklist_path.display(),
        repo.display(),
        index_path.display(),
        ledger_path.display()
    );
    fs::write(dir.path().join("config.toml"), &config_bytes).expect("backend config");
    let config = Config::load_from_path(dir.path().join("config.toml")).expect("config");
    let second_hash = "bb".repeat(32);
    let hashes = [CODE_HASH, second_hash.as_str()];
    let storage = Arc::new(GitSourceStorage::from_config(&config));
    for hash in hashes {
        store_mock_bundle(Arc::clone(&storage), hash).await;
    }
    let index = SqliteVerificationIndex::open(&index_path).expect("index");
    index
        .ensure_current(storage.as_ref())
        .await
        .expect("initial registry");
    assert_eq!(index.statistics().await.expect("statistics").total, 2);
    assert_eq!(
        index
            .abi_contracts(IndexedAbiContractsQuery {
                code_hash: None,
                limit: 10,
                offset: 0
            })
            .await
            .expect("ABI collection")
            .total,
        2
    );
    drop(index);
    drop(storage);

    drop(PaymentLedger::open(&ledger_path).expect("payment ledger"));
    let ledger = rusqlite::Connection::open(&ledger_path).expect("ledger fixture");
    ledger.execute(
        "insert into payment_transactions values (?1, ?2, 100, 1, 1600000000, 'consumed', 1600000000, 1)",
        rusqlite::params!["dd".repeat(32), CODE_HASH],
    ).expect("consumed payment");
    drop(ledger);
    let ledger_before = fs::read(&ledger_path).expect("ledger bytes");

    for (removed_index, hash) in hashes.into_iter().enumerate() {
        let index_before = fs::read(&index_path).expect("registry bytes");
        let bundle = bundle_path_for("sources", hash);
        let verify_message = git(&repo, &["log", "-1", "--format=%B", "--", &bundle]);
        assert_success(
            cli(&dir)
                .args(["remove-registry", hash])
                .output()
                .expect("run CLI"),
        );
        assert_eq!(
            git(&repo, &["show", "-s", "--format=%B"]),
            verify_message.replacen("Verify code hash", "Revoke verification for code hash", 1)
        );
        assert_eq!(fs::read(&index_path).expect("registry bytes"), index_before);
        assert_eq!(fs::read(&ledger_path).expect("ledger bytes"), ledger_before);
        assert_eq!(
            fs::read_to_string(dir.path().join("config.toml")).expect("config bytes"),
            config_bytes
        );

        // Reopen the same storage and persistent index, as backend startup does.
        let storage = GitSourceStorage::from_config(&config);
        let index = SqliteVerificationIndex::open(&index_path).expect("reopen registry");
        index
            .ensure_current(&storage)
            .await
            .expect("rebuild registry on restart");
        let remaining = hashes.len() - removed_index - 1;
        assert_eq!(
            index.statistics().await.expect("statistics").total,
            remaining
        );
        assert_eq!(
            index.last_verified(10, 0).await.expect("collection").total,
            remaining
        );
        for (position, query_hash) in hashes.into_iter().enumerate() {
            let present = position > removed_index;
            assert_eq!(
                index.status(query_hash).await.expect("status").verified,
                present
            );
            assert_eq!(
                index
                    .bundle(query_hash)
                    .await
                    .expect("indexed source")
                    .is_some(),
                present
            );
            assert_eq!(
                storage
                    .load_bundle(query_hash)
                    .await
                    .expect("stored source")
                    .is_some(),
                present
            );
            assert_eq!(
                index
                    .abi_contracts(IndexedAbiContractsQuery {
                        code_hash: Some(query_hash.to_owned()),
                        limit: 10,
                        offset: 0
                    })
                    .await
                    .expect("ABI")
                    .total,
                usize::from(present)
            );
        }
        assert_eq!(
            fs::read_to_string(&blocklist_path).expect("preserved blocklist"),
            CODE_HASH
        );
        let policy =
            CodeHashPolicy::from_file(config.blocked_code_hashes_path().expect("blocklist path"))
                .expect("blocklist");
        assert!(policy.is_blocked(CODE_HASH));
        assert!(!policy.is_blocked(&second_hash));
    }
    let ledger = rusqlite::Connection::open(&ledger_path).expect("reopen ledger");
    let state: String = ledger
        .query_row(
            "select state from payment_transactions where transaction_hash = ?1",
            ["dd".repeat(32)],
            |row| row.get(0),
        )
        .expect("payment state");
    assert_eq!(state, "consumed");
}

async fn store_mock_bundle(storage: Arc<GitSourceStorage>, code_hash: &str) {
    let compiler = support::mock_compiler::MockCompilerService::with_generated_sources(
        code_hash,
        vec![CompileGeneratedSource {
            path: "main.abi.json".to_owned(),
            content: r#"{"name":"Fixture","get_methods":[]}"#.to_owned(),
        }],
    );
    let state = AppState::new(
        Arc::new(support::mock_blockchain::MockBlockchainClient::new(&[])),
        Arc::new(compiler),
        Arc::new(SourceVerificationRegistry::new(
            storage,
            Arc::new(SqliteVerificationIndex::in_memory().expect("fixture index")),
        )),
        Arc::new(OnchainPaymentVerifier::new(
            Arc::new(support::StaticPaymentBlockchainClient::new(
                None,
                Vec::new(),
            )),
            PaymentLedger::in_memory().expect("fixture ledger"),
            support::PAYMENT_ADDRESS.to_owned(),
            1,
        )),
    )
    .with_api_key(Some("fixture-key"));
    let response = support::post_verify_with_api_key(
        state,
        vec![
            support::owned_text_part("code_hash", code_hash),
            support::text_part("language", "tolk"),
            support::text_part("compile_params", r#"{"compiler_version":"1.4.2"}"#),
            support::text_part("sources", r#"[{"path":"main.tolk","is_entrypoint":true}]"#),
            support::file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
        "fixture-key",
    )
    .await;
    let status = response.status();
    let body = support::response_json::<serde_json::Value>(response).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["verification_result"], "match");
}

#[cfg(unix)]
#[test]
fn commit_failure_restores_current_bundle_and_preserves_completed_commits() {
    let second_hash = "bb".repeat(32);
    let third_hash = "cc".repeat(32);
    for failed_hash in [CODE_HASH, second_hash.as_str()] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        for hash in [&second_hash, &third_hash] {
            let path = repo.join(bundle_path_for("sources", hash));
            fs::create_dir_all(&path).expect("bundle directory");
            fs::write(path.join("source.tolk"), "// source").expect("source");
        }
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "-m", "Add more bundles"]);
        git(&repo, &["push", "origin", "HEAD:main"]);
        let original = git(&repo, &["rev-parse", "HEAD"]);
        let failed_bundle = repo.join(bundle_path_for("sources", failed_hash));
        let before = directory_entries(&failed_bundle);
        let push_marker = dir.path().join("push-attempted");
        let output = cli_with_git_script(
            &dir,
            r#"
if [ "$1" = commit ]; then
    case "$4" in
        *"$CLI_TEST_FAIL_HASH"*) echo 'injected commit failure' >&2; exit 1 ;;
    esac
fi
if [ "$1" = push ]; then touch "$CLI_TEST_PUSH_MARKER"; fi
"#,
        )
        .env("CLI_TEST_FAIL_HASH", failed_hash)
        .env("CLI_TEST_PUSH_MARKER", &push_marker)
        .args(["remove-registry", CODE_HASH, &second_hash, &third_hash])
        .output()
        .expect("run CLI");
        assert_failure(&output, "injected commit failure");
        assert_failure(
            &output,
            "planned tracked files and index entries were restored",
        );
        assert_eq!(directory_entries(&failed_bundle), before);
        assert!(repo.join(bundle_path_for("sources", &third_hash)).exists());
        assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
        assert_eq!(
            git(
                &dir.path().join("source-repo.git"),
                &["rev-parse", "refs/heads/main"]
            ),
            original
        );
        assert!(!push_marker.exists());
        if failed_hash == CODE_HASH {
            assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
        } else {
            assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
            assert!(!repo.join(bundle_path("sources")).exists());
            assert_failure(&output, &git(&repo, &["rev-parse", "HEAD"]));
            assert_failure(&output, "earlier local commits are preserved");
        }
    }
}

#[cfg(unix)]
#[test]
fn commit_created_before_command_failure_is_preserved() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let output = cli_with_git_script(
        &dir,
        r#"
if [ "$1" = commit ]; then
    "$CLI_TEST_REAL_GIT" "$@"
    echo 'failure after commit' >&2
    exit 1
fi
"#,
    )
    .args(["remove-registry", CODE_HASH])
    .output()
    .expect("run CLI");
    assert_failure(&output, "failure after commit");
    assert_failure(&output, "local commit preserved, push was not attempted");
    assert_failure(&output, &git(&repo, &["rev-parse", "HEAD"]));
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
    assert!(!repo.join(bundle_path("sources")).exists());
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        original
    );
}

#[cfg(unix)]
#[test]
fn failed_head_check_skips_recovery_after_a_commit_error() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let output = cli_with_git_script(
        &dir,
        r#"
if [ "$1" = commit ]; then
    "$CLI_TEST_REAL_GIT" "$@"
    touch "$CLI_TEST_MARKER"
    echo 'failure after commit' >&2
    exit 1
fi
if [ "$1" = rev-parse ] && [ -f "$CLI_TEST_MARKER" ]; then
    echo 'HEAD unavailable' >&2
    exit 1
fi
"#,
    )
    .env("CLI_TEST_MARKER", dir.path().join("committed"))
    .args(["remove-registry", CODE_HASH])
    .output()
    .expect("run CLI");
    assert_failure(&output, "failure after commit");
    assert_failure(&output, "HEAD unavailable");
    assert_failure(&output, "recovery was skipped");
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
    assert!(!repo.join(bundle_path("sources")).exists());
}

#[cfg(unix)]
#[test]
fn push_failure_preserves_commits_even_when_remote_accepted_them() {
    for accepted in [false, true] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let remote = dir.path().join("source-repo.git");
        let original = git(&repo, &["rev-parse", "HEAD"]);
        let output = cli_with_git_script(
            &dir,
            r#"
if [ "$1" = push ]; then
    if [ "$CLI_TEST_ACCEPTED" = true ]; then "$CLI_TEST_REAL_GIT" "$@"; fi
    echo 'injected push failure' >&2
    exit 1
fi
"#,
        )
        .env("CLI_TEST_ACCEPTED", accepted.to_string())
        .args(["remove-registry", CODE_HASH])
        .output()
        .expect("run CLI");
        let head = git(&repo, &["rev-parse", "HEAD"]);
        assert_failure(&output, "injected push failure");
        assert_failure(&output, &format!("local commits are preserved at {head}"));
        assert_failure(&output, "inspect the remote before retrying publication");
        assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
        assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
        assert!(!repo.join(bundle_path("sources")).exists());
        assert_eq!(
            git(&remote, &["rev-parse", "refs/heads/main"]),
            if accepted { head } else { original }
        );
    }
}

#[cfg(unix)]
#[test]
fn remote_advancing_after_checks_rejects_push_without_force() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let remote = dir.path().join("source-repo.git");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let tree = git(&repo, &["show", "-s", "--format=%T", "HEAD"]);
    let competing = git(
        &repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &original,
            "-m",
            "Concurrent publication",
        ],
    );
    git(
        &repo,
        &[
            "push",
            "origin",
            &format!("{competing}:refs/heads/competing"),
        ],
    );
    let output = cli_with_git_script(
        &dir,
        r#"
if [ "$1" = push ]; then
    "$CLI_TEST_REAL_GIT" -C "$CLI_TEST_REMOTE" update-ref refs/heads/main "$CLI_TEST_COMPETING"
fi
"#,
    )
    .env("CLI_TEST_REMOTE", &remote)
    .env("CLI_TEST_COMPETING", &competing)
    .args(["remove-registry", CODE_HASH])
    .output()
    .expect("run CLI");
    assert_failure(&output, "push to origin/main failed");
    assert_failure(&output, &git(&repo, &["rev-parse", "HEAD"]));
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/main"]), competing);
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), original);
    assert_eq!(git(&repo, &["status", "--porcelain=v1"]), "");
}

#[cfg(unix)]
fn executable_script(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).expect("test script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable script");
}

#[cfg(unix)]
fn cli_with_git_script(dir: &TempDir, script: &str) -> Command {
    let original_path = std::env::var_os("PATH").expect("PATH");
    let paths = std::env::split_paths(&original_path).collect::<Vec<_>>();
    let real_git = paths
        .iter()
        .map(|path| path.join("git"))
        .find(|path| path.is_file())
        .expect("Git executable");
    let scripts = dir.path().join("test-bin");
    fs::create_dir(&scripts).expect("test scripts directory");
    executable_script(
        &scripts.join("git"),
        &format!("#!/bin/sh\nset -eu\n{script}\nexec \"$CLI_TEST_REAL_GIT\" \"$@\"\n"),
    );
    let path = std::env::join_paths(std::iter::once(scripts).chain(paths)).expect("test PATH");
    let mut command = cli(dir);
    command.env("PATH", path).env("CLI_TEST_REAL_GIT", real_git);
    command
}

fn bundle_path(storage_root: &str) -> String {
    bundle_path_for(storage_root, CODE_HASH)
}

fn bundle_path_for(storage_root: &str, hash: &str) -> String {
    format!("{storage_root}/{}/{}", &hash[..2], &hash[2..])
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Verifier CLI tests")
        .env("GIT_AUTHOR_EMAIL", "verifier@example.invalid")
        .env("GIT_COMMITTER_NAME", "Verifier CLI tests")
        .env("GIT_COMMITTER_EMAIL", "verifier@example.invalid")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .expect("run Git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF-8 Git output")
        .trim()
        .to_owned()
}

fn prepare_repository(
    dir: &TempDir,
    name: &str,
    storage_root: &str,
    remote: &str,
    branch: &str,
) -> PathBuf {
    let repo = dir.path().join(name);
    fs::create_dir(&repo).expect("checkout directory");
    git(&repo, &["init", "-b", branch]);
    git(&repo, &["config", "maintenance.auto", "false"]);
    fs::write(
        repo.join(".gitattributes"),
        format!("{storage_root}/** -text\n"),
    )
    .expect("initial attributes");
    git(&repo, &["add", ".gitattributes"]);
    git(&repo, &["commit", "-m", "Initialize source repository"]);
    let bundle = repo.join(bundle_path(storage_root));
    fs::create_dir_all(&bundle).expect("bundle directory");
    fs::write(bundle.join("source.tolk"), "// source fixture\n").expect("source file");
    // Checkout checks do not require a valid manifest or invoke compilers.
    fs::write(bundle.join("manifest.json"), "broken manifest\n").expect("manifest file");
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add fixture bundle"]);
    let remote_path = dir.path().join(format!("{name}.git"));
    fs::create_dir(&remote_path).expect("remote directory");
    git(&remote_path, &["init", "--bare"]);
    git(&remote_path, &["config", "receive.autogc", "false"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            remote,
            remote_path.to_str().expect("remote path"),
        ],
    );
    git(&repo, &["push", remote, &format!("HEAD:{branch}")]);
    repo
}

#[derive(Debug, PartialEq, Eq)]
enum EntrySnapshot {
    Directory,
    File(Vec<u8>, u32),
    Symlink(PathBuf),
}

fn directory_entries(path: &Path) -> BTreeMap<PathBuf, EntrySnapshot> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(path).expect("read fixture directory") {
        let entry = entry.expect("directory entry");
        let kind = entry.file_type().expect("entry type");
        let snapshot = if kind.is_dir() {
            entries.extend(directory_entries(&entry.path()));
            EntrySnapshot::Directory
        } else if kind.is_symlink() {
            EntrySnapshot::Symlink(fs::read_link(entry.path()).expect("symlink target"))
        } else {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                entry
                    .metadata()
                    .expect("file metadata")
                    .permissions()
                    .mode()
            };
            #[cfg(not(unix))]
            let mode = 0;
            EntrySnapshot::File(fs::read(entry.path()).expect("file bytes"), mode)
        };
        entries.insert(entry.path(), snapshot);
    }
    entries
}

fn run_preserving(dir: &TempDir, repo: &Path, args: &[&str]) -> Output {
    run_preserving_command(dir, repo, cli(dir).args(args))
}

fn run_preserving_command(dir: &TempDir, repo: &Path, command: &mut Command) -> Output {
    let index = repo.join(".git/index");
    let staged_before = git(repo, &["ls-files", "--stage", "-z"]);
    let mut before = directory_entries(dir.path());
    // git status may refresh stat cache; compare staged contents and modes separately.
    before.remove(&index);
    let output = command.output().expect("run CLI");
    let mut after = directory_entries(dir.path());
    after.remove(&index);
    assert_eq!(
        after, before,
        "CLI must preserve checkout, refs, remote and databases"
    );
    assert_eq!(
        git(repo, &["ls-files", "--stage", "-z"]),
        staged_before,
        "CLI must preserve staged state"
    );
    output
}
