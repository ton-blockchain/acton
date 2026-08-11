use std::{
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use verifier::{
    config::Config,
    source_storage::{
        CompilerMetadata, GitSourceStorage, SourceMapData, SourceStorage, SourceStorageError,
        SourceStorageFile, StoreSourceBundleRequest,
    },
};

const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SOURCE_BUNDLE_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SECOND_BUNDLE_HASH: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const ORIGINAL_VERIFIED_AT: u64 = 1_600_000_000;

#[tokio::test]
async fn git_source_storage_uses_configured_storage_root() -> Result<(), Box<dyn Error>> {
    let storage_root = "verified/contracts";
    let fixture = GitFixture::new_with_storage_root(storage_root)?;
    let config_path = fixture.write_config_with_storage_root(storage_root)?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    assert!(storage.current_revision().await?.is_some());
    let receipt = storage
        .store_bundle(StoreSourceBundleRequest {
            code_hash: CODE_HASH.to_owned(),
            source_bundle_hash: SOURCE_BUNDLE_HASH.to_owned(),
            payment_tx_hash: None,
            verified_at: Some(ORIGINAL_VERIFIED_AT),
            compiler: CompilerMetadata {
                language: "tolk".to_owned(),
                version: "1.4.1".to_owned(),
                entrypoint: "main.tolk".to_owned(),
                params: json!({"compiler_version": "1.4.1"}),
            },
            files: vec![SourceStorageFile {
                path: "main.tolk".to_owned(),
                content: "fun main() {}".to_owned(),
                include_in_command: None,
                is_stdlib: None,
                has_include_directives: None,
            }],
            source_map: None,
        })
        .await?;

    assert!(receipt.created);
    assert!(
        fixture
            .repo_path
            .join(storage_root)
            .join(CODE_HASH)
            .join("manifest.json")
            .is_file()
    );
    assert_eq!(storage.list_code_hashes().await?, vec![CODE_HASH]);
    let stored_bundle = storage
        .load_bundle(CODE_HASH)
        .await?
        .expect("stored bundle should exist");
    assert_eq!(stored_bundle.manifest.verified_at, ORIGINAL_VERIFIED_AT);
    assert_eq!(
        git_output(
            &fixture.repo_path,
            ["log", "-1", "--format=%at %ct", "--", storage_root]
        )?,
        format!("{ORIGINAL_VERIFIED_AT} {ORIGINAL_VERIFIED_AT}")
    );

    Ok(())
}

#[tokio::test]
async fn git_source_storage_rejects_untracked_files_at_startup() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    fs::write(fixture.repo_path.join("pending.txt"), "not committed\n")?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    let error = storage
        .current_revision()
        .await
        .expect_err("source repository should be clean at startup");
    assert!(matches!(
        &error,
        SourceStorageError::DirtySourceRepository { changes, .. }
            if changes == "?? pending.txt"
    ));

    Ok(())
}

#[tokio::test]
async fn git_source_storage_rejects_unstaged_changes_at_startup() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    fs::write(
        fixture.repo_path.join(".gitattributes"),
        "sources/** binary\n",
    )?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    let error = storage
        .current_revision()
        .await
        .expect_err("source repository should be clean at startup");
    assert!(matches!(
        &error,
        SourceStorageError::DirtySourceRepository { changes, .. }
            if changes == " M .gitattributes"
    ));

    Ok(())
}

#[tokio::test]
async fn git_source_storage_rejects_staged_changes_at_startup() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    fs::write(fixture.repo_path.join("pending.txt"), "not committed\n")?;
    assert_success(
        run_command(&fixture.repo_path, "git", ["add", "pending.txt"])?,
        "git add pending.txt",
    )?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    let error = storage
        .current_revision()
        .await
        .expect_err("source repository should be clean at startup");
    assert!(matches!(
        &error,
        SourceStorageError::DirtySourceRepository { changes, .. }
            if changes == "A  pending.txt"
    ));

    Ok(())
}

#[tokio::test]
async fn git_source_storage_accepts_any_root_commit_message() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "-c",
                "user.name=Verifier Bot",
                "-c",
                "user.email=verifier@example.com",
                "commit",
                "--amend",
                "--no-gpg-sign",
                "-m",
                "Unexpected root commit",
            ],
        )?,
        "git commit --amend",
    )?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    assert!(storage.current_revision().await?.is_some());

    Ok(())
}

#[tokio::test]
async fn git_source_storage_requires_initial_attributes_content() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    fs::write(
        fixture.repo_path.join(".gitattributes"),
        "sources/** text\n",
    )?;
    assert_success(
        run_command(&fixture.repo_path, "git", ["add", ".gitattributes"])?,
        "git add .gitattributes",
    )?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "-c",
                "user.name=Verifier Bot",
                "-c",
                "user.email=verifier@example.com",
                "commit",
                "--amend",
                "--no-gpg-sign",
                "-m",
                "Any root commit message",
            ],
        )?,
        "git commit --amend",
    )?;
    fs::write(
        fixture.repo_path.join(".gitattributes"),
        "sources/** -text\n",
    )?;
    assert_success(
        run_command(&fixture.repo_path, "git", ["add", ".gitattributes"])?,
        "git add .gitattributes",
    )?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "-c",
                "user.name=Verifier Bot",
                "-c",
                "user.email=verifier@example.com",
                "commit",
                "--no-gpg-sign",
                "-m",
                "Restore source attributes",
            ],
        )?,
        "git commit",
    )?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    let error = storage
        .current_revision()
        .await
        .expect_err("source repository should require the initial attributes content");
    assert!(matches!(
        error,
        SourceStorageError::UnpreparedSourceRepository { .. }
    ));

    Ok(())
}

#[tokio::test]
async fn git_source_storage_requires_current_source_attributes_rule() -> Result<(), Box<dyn Error>>
{
    let fixture = GitFixture::new()?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    assert!(storage.current_revision().await?.is_some());

    fs::write(
        fixture.repo_path.join(".gitattributes"),
        "sources/** -text\nsources/** text\n",
    )?;
    assert_success(
        run_command(&fixture.repo_path, "git", ["add", ".gitattributes"])?,
        "git add .gitattributes",
    )?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "-c",
                "user.name=Verifier Bot",
                "-c",
                "user.email=verifier@example.com",
                "commit",
                "--no-gpg-sign",
                "-m",
                "Remove source attributes rule",
            ],
        )?,
        "git commit",
    )?;

    let error = storage
        .current_revision()
        .await
        .expect_err("source repository should retain sources/** -text");
    assert!(matches!(
        error,
        SourceStorageError::UnpreparedSourceRepository { .. }
    ));

    Ok(())
}

#[tokio::test]
async fn git_source_storage_commits_pushes_and_keeps_first_bundle() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    assert!(storage.current_revision().await?.is_some());

    let started_at = unix_timestamp()?;
    let bundle_path = format!("sources/{CODE_HASH}");
    let receipt = storage
        .store_bundle(StoreSourceBundleRequest {
            code_hash: CODE_HASH.to_owned(),
            source_bundle_hash: SOURCE_BUNDLE_HASH.to_owned(),
            payment_tx_hash: Some(
                "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b".to_owned(),
            ),
            verified_at: None,
            compiler: CompilerMetadata {
                language: "tolk".to_owned(),
                version: "1.4.1".to_owned(),
                entrypoint: "main.tolk".to_owned(),
                params: json!({"compiler_version": "1.4.1"}),
            },
            source_map: Some(source_map_data_fixture()),
            files: vec![
                SourceStorageFile {
                    path: "main.tolk".to_owned(),
                    content: "import \"imports/lib.tolk\";".to_owned(),
                    include_in_command: None,
                    is_stdlib: None,
                    has_include_directives: None,
                },
                SourceStorageFile {
                    path: "imports/lib.tolk".to_owned(),
                    content: "fun helper() {}".to_owned(),
                    include_in_command: None,
                    is_stdlib: None,
                    has_include_directives: None,
                },
            ],
        })
        .await?;

    assert_eq!(receipt.revision.len(), 40);
    assert!(receipt.created);

    let stored_main = fixture.repo_path.join(&bundle_path).join("files/main.tolk");
    let stored_lib = fixture
        .repo_path
        .join(&bundle_path)
        .join("files/imports/lib.tolk");
    let manifest_path = fixture.repo_path.join(&bundle_path).join("manifest.json");

    assert_eq!(
        fs::read_to_string(stored_main)?,
        "import \"imports/lib.tolk\";"
    );
    assert_eq!(fs::read_to_string(stored_lib)?, "fun helper() {}");

    let manifest_bytes = fs::read(manifest_path)?;
    let manifest_hash = hex::encode(Sha256::digest(&manifest_bytes));
    assert_eq!(
        git_output(&fixture.repo_path, ["log", "-1", "--format=%B"])?,
        format!(
            "Verify code hash {CODE_HASH}\n\ncode_hash: {CODE_HASH}\nsource_bundle_hash: {SOURCE_BUNDLE_HASH}\nmanifest_hash: {manifest_hash}"
        )
    );

    let manifest = serde_json::from_slice::<serde_json::Value>(&manifest_bytes)?;
    assert_eq!(manifest["code_hash"], CODE_HASH);
    assert_eq!(manifest["source_bundle_hash"], SOURCE_BUNDLE_HASH);
    assert_eq!(manifest["compiler"]["entrypoint"], "main.tolk");
    assert_eq!(manifest["compiler"]["version"], "1.4.1");
    assert!(
        manifest
            .as_object()
            .is_some_and(|value| value.contains_key("source_map"))
    );
    assert_eq!(
        manifest["source_map"]["code_boc64"],
        "te6cckEBAQEAAgAAAEysuc0="
    );
    assert_eq!(manifest["files"].as_array().map(Vec::len), Some(2));
    assert_eq!(manifest["files"][1]["path"], "main.tolk");
    let verified_at = manifest["verified_at"]
        .as_u64()
        .expect("manifest should include verification timestamp");
    assert!(
        (started_at..=unix_timestamp()?).contains(&verified_at),
        "verification timestamp should be recorded when the bundle is stored"
    );
    assert_eq!(
        git_output(
            &fixture.repo_path,
            ["log", "-1", "--format=%at %ct", "--", &bundle_path]
        )?,
        format!("{verified_at} {verified_at}")
    );

    let stored_bundle = storage
        .load_bundle(CODE_HASH)
        .await?
        .expect("stored bundle should exist");
    assert_eq!(stored_bundle.manifest.code_hash, CODE_HASH);
    assert_eq!(stored_bundle.manifest.verified_at, verified_at);
    assert_eq!(
        stored_bundle.manifest.source_bundle_hash,
        SOURCE_BUNDLE_HASH
    );
    assert_eq!(
        stored_bundle
            .manifest
            .source_map
            .as_ref()
            .map(|data| data.debug_marks_base64.as_str()),
        Some("te6cckEBAQEAAgAAAEysuc0=")
    );
    assert_eq!(stored_bundle.storage_revision, receipt.revision);
    assert_eq!(
        stored_bundle.manifest.payment_tx_hash.as_deref(),
        Some("a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b")
    );
    assert_eq!(stored_bundle.files.len(), 2);
    assert_eq!(stored_bundle.files[0].path, "imports/lib.tolk");
    assert_eq!(stored_bundle.files[0].content, "fun helper() {}");
    assert_eq!(stored_bundle.files[1].path, "main.tolk");
    assert_eq!(
        stored_bundle.files[1].content,
        "import \"imports/lib.tolk\";"
    );

    let second_receipt = storage
        .store_bundle(StoreSourceBundleRequest {
            code_hash: CODE_HASH.to_owned(),
            source_bundle_hash: SECOND_BUNDLE_HASH.to_owned(),
            payment_tx_hash: None,
            verified_at: None,
            compiler: CompilerMetadata {
                language: "tolk".to_owned(),
                version: "1.4.2".to_owned(),
                entrypoint: "replacement.tolk".to_owned(),
                params: json!({"compiler_version": "1.4.2"}),
            },
            source_map: None,
            files: vec![SourceStorageFile {
                path: "replacement.tolk".to_owned(),
                content: "fun replacement() {}".to_owned(),
                include_in_command: None,
                is_stdlib: None,
                has_include_directives: None,
            }],
        })
        .await?;

    assert!(!second_receipt.created);
    assert_eq!(second_receipt.revision, receipt.revision);
    assert!(
        fixture
            .repo_path
            .join(&bundle_path)
            .join("files/main.tolk")
            .exists()
    );
    assert!(
        fixture
            .repo_path
            .join(&bundle_path)
            .join("files/imports/lib.tolk")
            .exists()
    );
    assert!(
        !fixture
            .repo_path
            .join(&bundle_path)
            .join("files/replacement.tolk")
            .exists()
    );
    let original = storage
        .load_bundle(CODE_HASH)
        .await?
        .expect("original bundle should still exist");
    assert_eq!(original.manifest.source_bundle_hash, SOURCE_BUNDLE_HASH);
    assert_eq!(original.manifest.verified_at, verified_at);
    assert_eq!(original.manifest.compiler.version, "1.4.1");
    assert_eq!(original.files.len(), 2);

    let remote_head = git_output(
        fixture.temp.path(),
        [
            "--git-dir",
            fixture
                .remote_path
                .to_str()
                .expect("remote path should be UTF-8"),
            "rev-parse",
            "refs/heads/main",
        ],
    )?;
    assert_eq!(remote_head, receipt.revision);

    Ok(())
}

#[tokio::test]
async fn git_source_storage_cleans_up_after_commit_failure_and_allows_retry()
-> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    let config = Config::load_from_path(fixture.write_config()?)?;
    let storage = GitSourceStorage::from_config(&config);
    let initial_revision = storage
        .current_revision()
        .await?
        .expect("fixture should have an initial revision");
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            ["config", "commit.gpgsign", "true"],
        )?,
        "git config commit.gpgsign",
    )?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            ["config", "gpg.program", "/path/that/does/not/exist/gpg"],
        )?,
        "git config gpg.program",
    )?;

    let error = storage
        .store_bundle(source_bundle_request())
        .await
        .expect_err("commit should fail when the signing program is unavailable");
    assert!(matches!(error, SourceStorageError::Git { .. }));
    assert_eq!(
        git_output(&fixture.repo_path, ["rev-parse", "HEAD"])?,
        initial_revision
    );
    assert_eq!(
        git_output(
            &fixture.repo_path,
            ["status", "--porcelain=v1", "--untracked-files=all"]
        )?,
        ""
    );
    assert!(!fixture.repo_path.join("sources").join(CODE_HASH).exists());

    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            ["config", "--unset", "commit.gpgsign"],
        )?,
        "git config --unset commit.gpgsign",
    )?;
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            ["config", "--unset", "gpg.program"],
        )?,
        "git config --unset gpg.program",
    )?;

    let receipt = storage.store_bundle(source_bundle_request()).await?;
    assert!(receipt.created);
    assert!(storage.load_bundle(CODE_HASH).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn git_source_storage_rolls_back_after_push_failure_and_allows_retry()
-> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    let config = Config::load_from_path(fixture.write_config()?)?;
    let storage = GitSourceStorage::from_config(&config);
    let initial_revision = storage
        .current_revision()
        .await?
        .expect("fixture should have an initial revision");
    let unavailable_remote = fixture.temp.path().join("unavailable.git");
    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "remote",
                "set-url",
                "origin",
                path_str(&unavailable_remote)?,
            ],
        )?,
        "git remote set-url unavailable",
    )?;

    let error = storage
        .store_bundle(source_bundle_request())
        .await
        .expect_err("push should fail when the remote does not exist");
    assert!(matches!(error, SourceStorageError::Git { .. }));
    assert_eq!(
        git_output(&fixture.repo_path, ["rev-parse", "HEAD"])?,
        initial_revision
    );
    assert_eq!(
        git_output(
            &fixture.repo_path,
            ["status", "--porcelain=v1", "--untracked-files=all"]
        )?,
        ""
    );
    assert!(!fixture.repo_path.join("sources").join(CODE_HASH).exists());

    assert_success(
        run_command(
            &fixture.repo_path,
            "git",
            [
                "remote",
                "set-url",
                "origin",
                path_str(&fixture.remote_path)?,
            ],
        )?,
        "git remote set-url restored",
    )?;

    let receipt = storage.store_bundle(source_bundle_request()).await?;
    assert!(receipt.created);
    assert_eq!(
        git_output(
            fixture.temp.path(),
            [
                "--git-dir",
                path_str(&fixture.remote_path)?,
                "rev-parse",
                "refs/heads/main",
            ]
        )?,
        receipt.revision
    );

    Ok(())
}

fn source_bundle_request() -> StoreSourceBundleRequest {
    StoreSourceBundleRequest {
        code_hash: CODE_HASH.to_owned(),
        source_bundle_hash: SOURCE_BUNDLE_HASH.to_owned(),
        payment_tx_hash: Some(
            "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b".to_owned(),
        ),
        verified_at: Some(ORIGINAL_VERIFIED_AT),
        compiler: CompilerMetadata {
            language: "tolk".to_owned(),
            version: "1.4.2".to_owned(),
            entrypoint: "main.tolk".to_owned(),
            params: json!({"compiler_version": "1.4.2"}),
        },
        files: vec![SourceStorageFile {
            path: "main.tolk".to_owned(),
            content: "fun main() {}".to_owned(),
            include_in_command: None,
            is_stdlib: None,
            has_include_directives: None,
        }],
        source_map: None,
    }
}

fn unix_timestamp() -> Result<u64, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn source_map_data_fixture() -> SourceMapData {
    SourceMapData {
        code_boc64: "te6cckEBAQEAAgAAAEysuc0=".to_owned(),
        symbol_types_json: json!([]),
        debug_marks_json: json!([]),
        debug_marks_base64: "te6cckEBAQEAAgAAAEysuc0=".to_owned(),
    }
}

struct GitFixture {
    temp: TempDir,
    repo_path: PathBuf,
    remote_path: PathBuf,
}

impl GitFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_storage_root("sources")
    }

    fn new_with_storage_root(storage_root: &str) -> Result<Self, Box<dyn Error>> {
        let temp = TempDir::new()?;
        let repo_path = temp.path().join("repo");
        let remote_path = temp.path().join("remote.git");

        assert_success(
            run_command(
                temp.path(),
                "git",
                ["init", "--bare", path_str(&remote_path)?],
            )?,
            "git init --bare",
        )?;
        assert_success(
            run_command(
                temp.path(),
                "git",
                ["init", "-b", "main", path_str(&repo_path)?],
            )?,
            "git init -b main",
        )?;
        assert_success(
            run_command(
                &repo_path,
                "git",
                ["remote", "add", "origin", path_str(&remote_path)?],
            )?,
            "git remote add",
        )?;
        fs::write(
            repo_path.join(".gitattributes"),
            format!("{storage_root}/** -text\n"),
        )?;
        assert_success(
            run_command(&repo_path, "git", ["add", ".gitattributes"])?,
            "git add .gitattributes",
        )?;
        assert_success(
            run_command(
                &repo_path,
                "git",
                [
                    "-c",
                    "user.name=ton-verifier",
                    "-c",
                    "user.email=ton-verifier@example.invalid",
                    "commit",
                    "--no-gpg-sign",
                    "-m",
                    "Initialize verifier source repository",
                ],
            )?,
            "git commit initial attributes",
        )?;
        assert_success(
            run_command(&repo_path, "git", ["push", "-u", "origin", "main"])?,
            "git push initial attributes",
        )?;

        Ok(Self {
            temp,
            repo_path,
            remote_path,
        })
    }

    fn write_config(&self) -> Result<PathBuf, Box<dyn Error>> {
        self.write_config_with_storage_root("sources")
    }

    fn write_config_with_storage_root(
        &self,
        storage_root: &str,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let path = self.temp.path().join("verifier.toml");
        fs::write(
            &path,
            format!(
                r#"
[source_repository]
path = "{}"
remote = "origin"
storage_root = "{storage_root}"
branch = "main"
author_name = "Verifier Bot"
author_email = "verifier@example.com"
"#,
                self.repo_path.display()
            ),
        )?;
        Ok(path)
    }
}

fn path_str(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()).into())
}

fn run_command<I, S>(dir: &Path, program: &str, args: I) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(Command::new(program).args(args).current_dir(dir).output()?)
}

fn git_output<I, S>(dir: &Path, args: I) -> Result<String, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_command(dir, "git", args)?;
    let output = assert_success(output, "git output")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn assert_success(output: Output, command: &str) -> Result<Output, Box<dyn Error>> {
    if output.status.success() {
        return Ok(output);
    }

    Err(format!(
        "{command} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
    .into())
}
