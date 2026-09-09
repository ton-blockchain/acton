use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(super) struct WrapperArgs {
    #[arg(
        help = "Contract name to generate wrappers for",
        value_name = "CONTRACT_NAME",
        required_unless_present_any = ["all", "catalog"],
        conflicts_with_all = ["all", "catalog"],
        add = ArgValueCompleter::new(super::complete_contracts)
    )]
    pub contract_id: Option<String>,
    #[arg(
        long,
        help = "Generate wrappers for every contract defined in Acton.toml",
        conflicts_with_all = ["output", "test_output", "catalog"]
    )]
    pub all: bool,
    #[arg(
        long,
        short,
        help = "Output path for generated wrapper file",
        conflicts_with = "output_dir"
    )]
    pub output: Option<String>,
    #[arg(
        long,
        help = "Output directory for generated wrapper files",
        value_name = "DIR",
        conflicts_with = "output"
    )]
    pub output_dir: Option<String>,
    #[arg(
        long,
        short,
        help = "Generate a stub test file for contract",
        default_value = "false",
        help_heading = "Tests"
    )]
    pub test: bool,
    #[arg(
        long,
        help = "Output path for test file",
        help_heading = "Tests",
        requires = "test"
    )]
    pub test_output: Option<String>,
    #[arg(
        long,
        help = "Output directory for generated test file",
        value_name = "DIR",
        help_heading = "Tests",
        conflicts_with = "test_output",
        requires = "test"
    )]
    pub test_output_dir: Option<String>,
    #[arg(
        long,
        help = "Generate a TypeScript wrapper via gen-typescript-from-tolk",
        help_heading = "TypeScript",
        conflicts_with_all = ["test", "test_output", "test_output_dir"]
    )]
    pub ts: bool,
    #[arg(
        long,
        help = "Generate Go types, cell codecs, and getter codecs via tolk-abi-to-go",
        help_heading = "Go",
        conflicts_with_all = ["ts", "test", "test_output", "test_output_dir", "output"]
    )]
    pub go: bool,
    #[arg(
        long,
        help = "Generate Go codecs from an ABI catalog without compiling sources",
        value_name = "FILE",
        help_heading = "Go",
        requires = "go"
    )]
    pub catalog: Option<PathBuf>,
    #[arg(
        long,
        help = "Go package name (default: [wrappers.go].package or wrappers)",
        value_name = "NAME",
        help_heading = "Go",
        requires = "go"
    )]
    pub go_package: Option<String>,
    #[arg(
        long,
        help = "Go generator executable (default: [wrappers.go].generator or tolk-abi-to-go)",
        value_name = "PATH",
        help_heading = "Go",
        requires = "go"
    )]
    pub go_generator: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser, error::ErrorKind};

    #[derive(Debug, Parser)]
    struct Cli {
        #[command(flatten)]
        wrapper: WrapperArgs,
    }

    #[test]
    fn wrapper_cli_valid_selections() {
        Cli::command().debug_assert();
        for args in [
            vec!["Counter"],
            vec!["--all"],
            vec!["Counter", "--ts"],
            vec!["--all", "--ts"],
            vec!["Counter", "--test", "--test-output", "test.tolk"],
            vec!["--all", "--test", "--test-output-dir", "tests"],
            vec!["Counter", "--go"],
            vec!["--all", "--go", "--output-dir", "generated"],
            vec!["--catalog", "catalog.json", "--go"],
            vec![
                "--catalog",
                "catalog.json",
                "--go",
                "--go-package",
                "codecs",
                "--go-generator",
                "./my generator",
                "--output-dir",
                "go codecs",
            ],
        ] {
            Cli::try_parse_from(std::iter::once("wrapper").chain(args))
                .expect("valid wrapper arguments");
        }
    }

    #[test]
    fn wrapper_cli_requires_selection_and_go() {
        for args in [
            vec![],
            vec!["--go"],
            vec!["--ts"],
            vec!["--catalog", "catalog.json"],
            vec!["Counter", "--go-package", "codecs"],
            vec!["Counter", "--go-generator", "generator"],
            vec!["Counter", "--test-output", "test.tolk"],
            vec!["Counter", "--test-output-dir", "tests"],
        ] {
            let error = Cli::try_parse_from(std::iter::once("wrapper").chain(args)).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument, "{error}");
        }
    }

    #[test]
    fn wrapper_cli_conflicts() {
        for args in [
            vec!["Counter", "--all"],
            vec!["Counter", "--catalog", "catalog.json", "--go"],
            vec!["--all", "--catalog", "catalog.json", "--go"],
            vec!["Counter", "--go", "--ts"],
            vec!["--catalog", "catalog.json", "--ts", "--go"],
            vec!["Counter", "--go", "--test"],
            vec!["Counter", "--go", "--test-output", "test.tolk"],
            vec!["Counter", "--go", "--test-output-dir", "tests"],
            vec!["Counter", "--go", "--output", "wrapper.go"],
            vec!["--all", "--go", "--output", "wrapper.go"],
            vec![
                "--catalog",
                "catalog.json",
                "--go",
                "--output",
                "wrapper.go",
            ],
            vec!["Counter", "--ts", "--test"],
            vec!["Counter", "--ts", "--test-output", "test.tolk"],
            vec!["Counter", "--ts", "--test-output-dir", "tests"],
            vec!["--all", "--output", "wrapper.tolk"],
            vec!["--all", "--test", "--test-output", "test.tolk"],
            vec![
                "Counter",
                "--output",
                "wrapper.tolk",
                "--output-dir",
                "wrappers",
            ],
            vec![
                "Counter",
                "--test",
                "--test-output",
                "test.tolk",
                "--test-output-dir",
                "tests",
            ],
        ] {
            let error = Cli::try_parse_from(std::iter::once("wrapper").chain(args)).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{error}");
        }
    }
}
