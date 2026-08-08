use zed_extension_api as zed;

struct TonExtension;

impl TonExtension {
    fn acton_command_for(worktree: &zed::Worktree) -> zed::Command {
        if let Some(acton) = worktree.which("acton") {
            return zed::Command {
                command: acton,
                args: vec!["ls".into(), "--stdio".into()],
                env: vec![],
            };
        }

        let manifest = format!("{}/Cargo.toml", worktree.root_path());
        zed::Command {
            command: "cargo".into(),
            args: vec![
                "run".into(),
                "--quiet".into(),
                "--manifest-path".into(),
                manifest,
                "--bin".into(),
                "acton".into(),
                "--".into(),
                "ls".into(),
                "--stdio".into(),
            ],
            env: vec![],
        }
    }
}

impl zed::Extension for TonExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(Self::acton_command_for(worktree))
    }
}

zed::register_extension!(TonExtension);
