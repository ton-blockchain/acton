use acton::commands;
use acton::commands::build::{BuildCommandOptions, build_cmd};
use acton::commands::check::check_cmd;
use acton::commands::compile::compile_cmd;
use acton::commands::create_app::DEFAULT_APP_DIR;
use acton::commands::disasm::disasm_cmd;
use acton::commands::doc::doc_tvm_cmd;
use acton::commands::docgen::docgen_cmd;
use acton::commands::doctor::doctor_cmd;
use acton::commands::fmt::fmt_cmd;
use acton::commands::func2tolk::{default_func2tolk_version, func2tolk_cmd};
use acton::commands::help::print_command_manual;
use acton::commands::hooks::{HooksCommand, hooks_cmd};
use acton::commands::init::init_cmd;
use acton::commands::internal::internal_register_contract;
use acton::commands::library::{fetch_cmd, info_cmd, publish_cmd};
use acton::commands::ls::ls_cmd;
use acton::commands::meta::{BuiltinSchema, print_schema_cmd};
use acton::commands::new::{ProjectTemplate, new_cmd};
use acton::commands::retrace::retrace_cmd;
use acton::commands::rpc::{RpcCommand, rpc_cmd};
use acton::commands::run::run_cmd;
use acton::commands::script::script_cmd;
use acton::commands::test::{mutation, test_cmd};
use acton::commands::up::up_cmd;
use acton::commands::verify::verify_cmd;
use acton::commands::wallet::{WalletCommand, wallet_cmd};
use acton::commands::wrapper::wrapper_cmd;
use acton::paths;
use acton_config::color::OwoColorize;
use acton_config::color::{ColorMode, init_color_mode};
use acton_config::config::{
    ActonConfig, CheckOutputFormat, Explorer, LocalnetSettings, Network, ResolutionSource,
    TestSettings, WalletsFile, global_wallets_path, init_manifest_path_with_source,
    init_project_root_with_source, manifest_path as configured_manifest_path,
    project_root as configured_project_root,
};
use acton_config::test::{
    BacktraceMode, CoverageFormat, MutationDiffMode, MutationLevel, ReportFormat, TestConfig,
};
use clap::ArgAction;
use clap::builder::styling::{AnsiColor, Color, Style};
use clap::builder::{StyledStr, Styles};
use clap::{ColorChoice, CommandFactory, FromArgMatches};
use clap::{Parser, Subcommand};
use clap_complete::CompleteEnv;
use clap_complete::engine::{
    ArgValueCompleter, CompletionCandidate, PathCompleter, ValueCompleter,
};
use commands::common::error_fmt;
use dotenvy::dotenv;
use human_panic::{Metadata, setup_panic};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, fs, process};
use tasm_core::printer::FormatOptions;
use tolk_compiler::SourceMap;

#[derive(Parser)]
#[command(
    name = "acton",
    version = get_acton_version(),
    disable_help_subcommand = true
)]
#[command(about = "TON blockchain development tool")]
#[command(color = ColorChoice::Auto)]
struct Cli {
    #[arg(
        long,
        value_enum,
        global = true,
        value_name = "WHEN",
        default_value_t = ColorMode::Auto,
        help = "Control when to use colored output"
    )]
    color: ColorMode,

    #[arg(long, global = true, value_name = "PATH", help = "Path to Acton.toml")]
    manifest_path: Option<PathBuf>,
    #[arg(
        long = "project-root",
        global = true,
        value_name = "PATH",
        help = "Path to project root",
        conflicts_with = "manifest_path"
    )]
    project_root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Add Acton support to current directory",
        long_about = "Initialize Acton support in the current directory. This is useful for adding Acton support to an existing project. With --create-dapp, Acton skips project initialization and only scaffolds a TypeScript app. With --stdlib-only, Acton only refreshes the bundled standard library.",
        after_help = detailed_help_pointer("init")
    )]
    Init {
        #[arg(
            long = "create-dapp",
            value_name = "PATH",
            num_args = 0..=1,
            default_missing_value = DEFAULT_APP_DIR,
            conflicts_with = "stdlib_only",
            help = "Create a TypeScript app scaffold in PATH (default: app)"
        )]
        create_dapp: Option<PathBuf>,
        #[arg(
            long,
            help = "Update the bundled standard library without touching Acton.toml"
        )]
        stdlib_only: bool,
    },
    #[command(
        about = "Create a new project from a template",
        long_about = "Create a new project in a specified directory. This will create a new directory with a basic project template.",
        after_help = detailed_help_pointer("new")
    )]
    New {
        #[arg(
            help = "Directory to create the project in (use '.' for the current directory)",
            required_unless_present = "templates"
        )]
        path: Option<String>,
        #[arg(long, help = "Project name")]
        name: Option<String>,
        #[arg(long, help = "Project description")]
        description: Option<String>,
        #[arg(long, value_enum, help = "Project template")]
        template: Option<ProjectTemplate>,
        #[arg(long, help = "License")]
        license: Option<String>,
        #[arg(
            long,
            help = "Include the template's TypeScript app scaffold when available"
        )]
        app: bool,
        #[arg(long, help = "Create and install the default project-local Git hooks")]
        hooks: bool,
        #[arg(long, help = "Include an AGENTS.md file with coding-agent guidance")]
        agents: bool,
        #[arg(
            long,
            hide = true,
            help = "Print machine-readable template metadata as JSON",
            conflicts_with_all = [
                "path",
                "name",
                "description",
                "template",
                "license",
                "app",
                "hooks",
                "agents"
            ]
        )]
        templates: bool,
    },
    #[command(
        about = "Show top-level or command-specific help",
        after_help = detailed_help_pointer("help")
    )]
    Help {
        #[arg(help = "Top-level command to get help for", add = ArgValueCompleter::new(complete_commands))]
        command: Option<String>,
    },
    #[command(
        about = "Manage project and global wallets",
        after_help = detailed_help_pointer("wallet")
    )]
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    #[command(
        about = "Install and manage project Git hooks",
        after_help = detailed_help_pointer("hooks")
    )]
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },
    #[command(
        about = "Inspect remote accounts and contracts",
        after_help = detailed_help_pointer("rpc")
    )]
    Rpc {
        #[command(subcommand)]
        command: RpcCommand,
    },
    #[command(
        about = "Run tests from a file or directory",
        after_help = detailed_help_pointer("test")
    )]
    Test {
        #[arg(help = "Test file or directory containing test files (default: project root)", add = ArgValueCompleter::new(PathCompleter::any()))]
        path: Option<String>,
        // Filtering
        #[arg(
            short,
            long,
            help = "Filter tests by regex pattern",
            help_heading = "Filtering"
        )]
        filter: Option<String>,
        #[arg(
            long,
            help = "Exclude test files and directories matching glob patterns",
            help_heading = "Filtering"
        )]
        exclude: Vec<String>,
        #[arg(
            long,
            help = "Include only test files and directories matching glob patterns",
            help_heading = "Filtering"
        )]
        include: Vec<String>,

        // Execution
        #[arg(
            long,
            help = "Stop executing tests after the first failure (default: [test].fail-fast or false)",
            help_heading = "Execution",
            num_args = 0..=1,
            default_missing_value = "true",
            require_equals = true
        )]
        fail_fast: Option<bool>,
        #[arg(
            long,
            value_name = "SEED",
            help = "Seed for reproducible fuzz runs",
            help_heading = "Execution"
        )]
        fuzz_seed: Option<u64>,
        #[arg(
            long,
            action = ArgAction::Count,
            help = "Increase executor log verbosity (currently supports only level 1)",
            help_heading = "Execution"
        )]
        verbose: u8,

        // Debugging
        #[arg(long, help = "Enable debug mode", help_heading = "Debugging")]
        debug: bool,
        #[arg(long, help = "Debug server port", help_heading = "Debugging")]
        debug_port: Option<u16>,
        #[arg(long, help = "Enable backtraces", help_heading = "Debugging")]
        backtrace: Option<BacktraceMode>,

        // Coverage
        #[arg(long, help = "Generate a coverage profile", help_heading = "Coverage")]
        coverage: bool,
        #[arg(
            long,
            help = "Output coverage profile in specified format",
            help_heading = "Coverage"
        )]
        coverage_format: Option<CoverageFormat>,
        #[arg(
            long,
            help = "Output coverage profile to specified file (default: lcov.info for lcov, coverage.txt for text)",
            help_heading = "Coverage"
        )]
        coverage_file: Option<String>,
        #[arg(
            long,
            value_name = "PERCENT",
            value_parser = parse_coverage_percent,
            help = "Fail if the final coverage score is below this percentage",
            help_heading = "Coverage"
        )]
        coverage_minimum_percent: Option<f64>,
        #[arg(
            long,
            help = "Include files from the @wrappers mapping in coverage reports",
            help_heading = "Coverage"
        )]
        coverage_include_wrappers: bool,
        #[arg(
            long,
            help = "Include files under tests/ and .test.tolk files in coverage reports",
            help_heading = "Coverage"
        )]
        coverage_include_tests: bool,

        // Profiling
        #[arg(
            long,
            help = "Create JSON snapshot of gas usage statistics",
            help_heading = "Profiling"
        )]
        snapshot: Option<String>,
        #[arg(
            long,
            help = "Compare gas usage with baseline snapshot file",
            help_heading = "Profiling"
        )]
        baseline_snapshot: Option<String>,
        #[arg(
            long,
            help = "Exit with non-zero code when profiling differs from baseline snapshot",
            help_heading = "Profiling",
            requires = "baseline_snapshot"
        )]
        fail_on_diff: bool,

        // Reporting
        #[arg(
            long,
            help = "Report formats to use",
            value_delimiter = ',',
            help_heading = "Reporting"
        )]
        reporter: Vec<ReportFormat>,
        #[arg(
            long,
            help = "Show decoded message bodies in printed transaction trees when ABI is known",
            help_heading = "Reporting"
        )]
        show_bodies: bool,
        #[arg(
            long,
            help = "JUnit XML output directory (default: [test].junit-path or test-results)",
            help_heading = "Reporting"
        )]
        junit_path: Option<String>,
        #[arg(
            long,
            help = "Merge all test suites into a single JUnit XML file",
            help_heading = "Reporting"
        )]
        junit_merge: bool,

        // Cache
        #[arg(
            long,
            help = "Clear compilation cache before running (default: false)",
            help_heading = "Cache",
            num_args = 0..=1,
            default_missing_value = "true",
            require_equals = true
        )]
        clear_cache: Option<bool>,

        // Remote
        #[arg(
            long,
            help = "Fork from network for remote account resolution",
            help_heading = "Remote"
        )]
        fork_net: Option<String>,
        #[arg(
            long,
            help = "Block sequence number to fork from (for historical state)",
            value_name = "SEQNO",
            help_heading = "Remote"
        )]
        fork_block_number: Option<u64>,

        // Tracing
        #[arg(
            long,
            help = "Save transaction traces to directory",
            help_heading = "Tracing",
            value_name = "DIR",
            default_missing_value = "build/traces",
            num_args = 0..=1,
        )]
        save_test_trace: Option<String>,

        // Mutation testing
        #[arg(
            long,
            help = "Run tests in mutation testing mode",
            help_heading = "Mutation Testing"
        )]
        mutate: bool,
        #[arg(
            long,
            help = "Internal flag for overrides",
            help_heading = "Mutation Testing",
            hide = true
        )]
        mutate_overrides: Option<String>,
        #[arg(
            long,
            help = "Contract to mutate during mutation testing",
            help_heading = "Mutation Testing",
            value_name = "CONTRACT_NAME"
        )]
        mutate_contract: Option<String>,
        #[arg(
            long,
            help = "Path to a JSON file with custom query-based mutation rules",
            help_heading = "Mutation Testing",
            value_name = "PATH"
        )]
        mutation_rules_file: Option<String>,
        #[arg(
            long,
            help = "Session ID used for mutation progress logging and resume",
            help_heading = "Mutation Testing",
            value_name = "ID"
        )]
        mutation_session_id: Option<String>,
        #[arg(
            long,
            value_parser = parse_mutation_workers,
            help = "Number of worker threads used for mutation testing (defaults to available parallelism)",
            help_heading = "Mutation Testing",
            value_name = "N"
        )]
        mutation_workers: Option<usize>,
        #[arg(
            long,
            value_enum,
            help = "Limit mutation testing to changed lines in the selected diff scope",
            help_heading = "Mutation Testing",
            value_name = "MODE"
        )]
        mutation_diff: Option<MutationDiffMode>,
        #[arg(
            long,
            help = "Base ref used by diff-based mutation testing modes",
            help_heading = "Mutation Testing",
            value_name = "REF"
        )]
        mutation_diff_ref: Option<String>,
        #[arg(
            long,
            value_enum,
            value_delimiter = ',',
            help = "Run only selected mutation levels (comma-separated)",
            help_heading = "Mutation Testing",
            value_name = "LEVEL[,LEVEL...]"
        )]
        mutation_levels: Vec<MutationLevel>,
        #[arg(
            long = "mutation-id",
            value_delimiter = ',',
            value_parser = parse_mutation_id,
            help = "Run only specific mutation IDs from a previous mutation report",
            help_heading = "Mutation Testing",
            value_name = "ID"
        )]
        id: Vec<usize>,
        #[arg(
            long,
            value_name = "PERCENT",
            value_parser = parse_mutation_percent,
            help = "Fail if mutation score is below this percentage",
            help_heading = "Mutation Testing"
        )]
        mutation_minimum_percent: Option<f64>,
        #[arg(
            long,
            help = "Disable specific mutation rules",
            help_heading = "Mutation Testing",
            value_name = "RULE"
        )]
        mutation_disable_rules: Vec<String>,
        #[arg(
            long,
            help = "Open test results in a browser",
            help_heading = "Reporting"
        )]
        ui: bool,
        #[arg(
            long,
            help = "UI server port (default: [test].ui-port or 12344)",
            help_heading = "Reporting",
            value_name = "PORT"
        )]
        ui_port: Option<u16>,
    },
    #[command(
        about = "Generate contract wrappers and test stubs",
        after_help = detailed_help_pointer("wrapper")
    )]
    Wrapper {
        #[arg(help = "Contract name to generate wrappers for", value_name = "CONTRACT_NAME", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: String,
        #[arg(
            long,
            short,
            help = "Output path for generated wrapper file",
            conflicts_with = "output_dir"
        )]
        output: Option<String>,
        #[arg(
            long,
            help = "Output directory for generated wrapper file",
            value_name = "DIR",
            conflicts_with = "output"
        )]
        output_dir: Option<String>,

        #[arg(
            long,
            short,
            help = "Generate a stub test file for contract",
            default_value = "false",
            help_heading = "Tests"
        )]
        test: bool,
        #[arg(
            long,
            help = "Output path for test file",
            help_heading = "Tests",
            requires = "test"
        )]
        test_output: Option<String>,
        #[arg(
            long,
            help = "Output directory for generated test file",
            value_name = "DIR",
            help_heading = "Tests",
            conflicts_with = "test_output",
            requires = "test"
        )]
        test_output_dir: Option<String>,
        #[arg(
            long,
            help = "Generate a TypeScript wrapper via gen-typescript-from-tolk",
            help_heading = "TypeScript",
            conflicts_with_all = ["test", "test_output", "test_output_dir"]
        )]
        ts: bool,
    },
    #[command(
        about = "Run a standalone Tolk script file",
        after_help = detailed_help_pointer("script")
    )]
    Script {
        #[arg(help = "Script file to execute", add = ArgValueCompleter::new(PathCompleter::file()))]
        path: String,

        #[arg(help = "Arguments to pass to the script")]
        args: Vec<String>,

        #[arg(
            long,
            action = ArgAction::Count,
            help = "Increase executor log verbosity (currently supports only level 1)",
            help_heading = "Script"
        )]
        verbose: u8,

        // Debugging
        #[arg(long, help = "Enable debug mode", help_heading = "Debugging")]
        debug: bool,
        #[arg(long, help = "Enable backtraces", help_heading = "Debugging")]
        backtrace: Option<BacktraceMode>,
        #[arg(
            long,
            help = "Debug server port",
            default_value = "12345",
            help_heading = "Debugging"
        )]
        debug_port: u16,

        // Cache
        #[arg(
            long,
            help = "Clear compilation cache before running",
            help_heading = "Cache"
        )]
        clear_cache: bool,

        // Remote
        #[arg(
            long,
            help = "Fork from network for remote account resolution",
            help_heading = "Remote"
        )]
        fork_net: Option<String>,
        #[arg(
            long,
            help = "Block sequence number to fork from (for historical state)",
            value_name = "SEQNO",
            help_heading = "Remote"
        )]
        fork_block_number: Option<u64>,

        // Broadcasting
        #[arg(
            long,
            help = "Broadcast to the selected network; if omitted, run in emulation mode",
            help_heading = "Broadcasting"
        )]
        net: Option<String>,
        #[arg(
            long,
            help = "Use TON Connect wallet approval for broadcast messages",
            help_heading = "Broadcasting"
        )]
        tonconnect: bool,
        #[arg(
            long,
            default_value_t = acton::tonconnect::DEFAULT_TONCONNECT_PORT,
            help = "Local TON Connect page port",
            help_heading = "Broadcasting"
        )]
        tonconnect_port: u16,

        #[arg(
            value_enum,
            long,
            help = "Explorer to use for transaction links",
            help_heading = "Broadcasting",
            value_name = "NAME"
        )]
        explorer: Option<Explorer>,
        #[arg(
            long,
            help = "Show decoded message bodies in printed transaction trees when ABI is known",
            help_heading = "Output"
        )]
        show_bodies: bool,
    },
    #[command(
        about = "Build one contract or every contract",
        after_help = detailed_help_pointer("build")
    )]
    Build {
        #[arg(help = "Contract name to build (defaults to all if not specified)", value_name = "CONTRACT_NAME", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Clear compilation cache before building")]
        clear_cache: bool,
        #[arg(long, help = "Generate dependency graph as DOT file")]
        graph: Option<String>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Output directory for build artifacts (default: build/)"
        )]
        out_dir: Option<String>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Output directory for generated dependency files (default: gen/)"
        )]
        gen_dir: Option<String>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Directory to save contract ABI JSON files (default: build/abi/)"
        )]
        output_abi: Option<String>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Directory to save compiled Fift files"
        )]
        output_fift: Option<String>,
        #[arg(long, help = "Show compiled contract info")]
        info: bool,
    },
    #[command(
        about = "Run a named script from Acton.toml",
        after_help = detailed_help_pointer("run")
    )]
    Run {
        #[arg(help = "Name of the command script to run", add = ArgValueCompleter::new(complete_scripts))]
        script: String,
        #[arg(
            help = "Arguments to pass to the script",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    #[command(
        about = "Compile one Tolk source into TVM code",
        after_help = detailed_help_pointer("compile")
    )]
    Compile {
        #[arg(help = "Tolk file to compile", add = ArgValueCompleter::new(PathCompleter::file()))]
        path: String,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
        #[arg(long, help = "Output only base64 code")]
        base64_only: bool,
        #[arg(long, help = "Output code to binary BoC file")]
        boc: Option<String>,
        #[arg(long, help = "Output Fift code to file")]
        fift: Option<String>,
        #[arg(long, help = "Output source map to file (enables debug compilation)")]
        source_map: Option<String>,
        #[arg(long, help = "Output ABI to file")]
        abi: Option<String>,
        #[arg(
            long,
            help = "Allow compiling files without `main()` or `onInternalMessage()` entrypoints"
        )]
        allow_no_entrypoint: bool,
        #[arg(long, help = "Clear compilation cache before running")]
        clear_cache: bool,
    },
    #[command(
        about = "Disassemble TVM code into TASM",
        after_help = detailed_help_pointer("disasm")
    )]
    Disasm {
        #[arg(
            help = "BoC file to disassemble, either binary or text with hex/base64 data (use -s for inline data)"
        )]
        boc_file: Option<String>,
        #[arg(short, long, help = "BoC string in hex or base64 format")]
        string: Option<String>,
        #[arg(
            short,
            long,
            help = "Write output to file (creates parent dirs, overwrites existing file)"
        )]
        output: Option<String>,
        #[arg(long, help = "Show cell hashes and offsets for each cell")]
        show_hashes: bool,
        #[arg(long, help = "Show instruction offsets in left column")]
        show_offsets: bool,
        #[arg(long, help = "Print machine-readable disassembly JSON to stdout")]
        json: bool,
        #[arg(long, help = "Source map JSON from `acton compile --source-map`")]
        source_map: Option<String>,
        #[arg(
            long,
            help = "Contract address to fetch from blockchain (e.g., UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM)"
        )]
        address: Option<String>,
        #[arg(long, help = "Network for `--address` and library lookups")]
        net: Option<String>,
        #[arg(
            long,
            help = "Follow library references and disassemble the actual library code instead of showing library hash"
        )]
        follow_libraries: bool,
    },
    #[command(
        about = "Verify contract source on TON Verifier",
        after_help = detailed_help_pointer("verify")
    )]
    Verify {
        #[arg(help = "Contract name to verify (prompts if not provided)", value_name = "CONTRACT_NAME", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Deployed contract address (prompts if not provided)")]
        address: Option<String>,
        #[arg(long, help = "Network to use", default_value = "testnet")]
        net: String,
        #[arg(
            long,
            help = "Wallet from Acton.toml to use for verification (defaults to the only one if single wallet configured)",
            add = ArgValueCompleter::new(complete_wallets)
        )]
        wallet: Option<String>,
        #[arg(long, help = "Tolk compiler version to use on verifier side")]
        compiler_version: Option<String>,
        #[arg(long, help = "Run verification without sending the final transaction")]
        dry_run: bool,
    },
    #[command(
        about = "Check project Tolk sources for errors",
        after_help = detailed_help_pointer("check")
    )]
    Check {
        #[arg(help = "Contract name to check or path to a .tolk file", add = ArgValueCompleter::new(complete_contracts_or_paths))]
        target: Option<String>,
        #[arg(long, help = "Automatically apply available fixes (plain output only)")]
        fix: bool,
        #[arg(
            long = "output-format",
            value_enum,
            value_name = "FORMAT",
            help = "Output format (plain, json, sarif, github, gitlab)"
        )]
        output_format: Option<CheckOutputFormat>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Write output result to file (default: stdout)"
        )]
        output_file: Option<PathBuf>,
        #[arg(
            long = "enable-only",
            value_delimiter = ',',
            value_name = "CODE[,CODE...]",
            help = "Enable only selected lint rules by code (e.g. E001,S001)"
        )]
        enable_only: Option<Vec<String>>,
        #[arg(long, help = "Explain a rule")]
        explain: Option<String>,
        #[arg(long, hide = true)]
        list_lint_rules: bool,
    },
    #[command(hide = true, disable_help_flag = true, disable_help_subcommand = true)]
    Lint {
        #[arg(
            value_name = "ARG",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    #[command(
        about = "Replay a transaction trace by hash",
        after_help = detailed_help_pointer("retrace")
    )]
    Retrace {
        #[arg(help = "Transaction hash in hex format to retrace")]
        hash: String,
        #[arg(long, help = "Network to use")]
        net: Option<String>,
        #[arg(long, help = "Show full cell hex instead of hashes in out actions")]
        verbose: bool,
        #[arg(long, help = "Directory to save VM and executor logs")]
        logs_dir: Option<String>,
        #[arg(
            long,
            help = "Contract name from Acton.toml used to build a source-level trace for the transaction",
            add = ArgValueCompleter::new(complete_contracts)
        )]
        contract: Option<String>,
        #[arg(
            long,
            help = "Enable source-level debugging for the retraced transaction; requires --contract",
            help_heading = "Debugging"
        )]
        debug: bool,
        #[arg(long, help = "Debug server port", help_heading = "Debugging")]
        debug_port: Option<u16>,
    },
    #[command(
        about = "Publish and manage on-chain libraries",
        after_help = detailed_help_pointer("library")
    )]
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
    #[command(
        about = "Manage local TON network",
        after_help = detailed_help_pointer("localnet")
    )]
    Localnet {
        #[command(subcommand)]
        command: LocalnetCommand,
    },
    #[command(
        about = "Format project Tolk source files",
        after_help = detailed_help_pointer("fmt")
    )]
    Fmt {
        #[arg(help = "Files or directories to format (defaults to project root)", add = ArgValueCompleter::new(PathCompleter::any()))]
        paths: Vec<String>,
        #[arg(long, help = "Check if files are formatted without overwriting them")]
        check: bool,
    },
    #[command(
        about = "Look up TVM reference documentation",
        after_help = detailed_help_pointer("doc")
    )]
    Doc {
        #[command(subcommand)]
        command: DocCommand,
    },
    #[command(
        about = "Run LSP server for the TON languages and technologies",
        after_help = detailed_help_pointer("ls")
    )]
    Ls {
        #[arg(long, help = "Port to listen on (TCP)")]
        port: Option<u16>,
        #[arg(long, help = "Use stdio for communication (default)")]
        stdio: bool,
        #[arg(long, help = "Path to log file")]
        log_file: Option<String>,
        #[arg(long, help = "Disable logging")]
        no_log: bool,
    },
    #[command(
        about = "Install or update Acton CLI releases",
        after_help = detailed_help_pointer("up")
    )]
    Up {
        #[arg(
            help = "Specific version to install",
            conflicts_with_all = ["trunk", "stable", "list", "check"]
        )]
        version: Option<String>,
        #[arg(
            long,
            help = "Install the most recent trunk release",
            conflicts_with_all = ["stable", "list", "check"]
        )]
        trunk: bool,
        #[arg(
            long,
            help = "Install the latest stable release",
            conflicts_with_all = ["list", "check"]
        )]
        stable: bool,
        #[arg(
            long,
            help = "Install the selected release even if Acton is already up to date",
            conflicts_with_all = ["list", "check"]
        )]
        force: bool,
        #[arg(long, help = "List available versions", conflicts_with = "check")]
        list: bool,
        #[arg(long, hide = true, help = "Check for updates and return info as JSON")]
        check: bool,
    },
    #[command(
        name = "func2tolk",
        about = "Convert FunC sources into Tolk code",
        after_help = detailed_help_pointer("func2tolk")
    )]
    Func2Tolk {
        #[arg(help = "Path to a .fc/.func file or a directory containing them")]
        path: String,
        #[arg(long, help = "Output path")]
        output: Option<String>,
        #[arg(
            long,
            help = "Insert /* _WARNING_ */ comments in output instead of printing warnings only"
        )]
        warnings_as_comments: bool,
        #[arg(long, help = "Don't transform snake_case to camelCase")]
        no_camel_case: bool,
        #[arg(
            long,
            default_value = default_func2tolk_version(),
            help = "Version of @ton/convert-func-to-tolk to use"
        )]
        version: String,
    },
    #[command(
        about = "Inspect the resolved project setup",
        after_help = detailed_help_pointer("doctor")
    )]
    Doctor,
    #[command(
        about = "Generate shell completion scripts",
        after_help = detailed_help_pointer("completions")
    )]
    Completions {
        #[arg(value_parser = ["bash", "elvish", "fish", "powershell", "zsh", "nushell"])]
        shell: String,
    },
    #[command(hide = true)]
    Meta {
        #[command(subcommand)]
        command: MetaCommand,
    },
    #[command(
        about = "Internal command to generate MDX documentation from standard library",
        hide = true
    )]
    Docgen {
        #[arg(short, long, help = "Output directory path")]
        output: Option<String>,
        #[arg(
            long,
            help = "Check if generated documentation is up to date without writing files"
        )]
        check: bool,
    },
    #[command(name = "internal-register-contract", hide = true)]
    InternalRegisterContract {
        #[arg(help = "Path to the contract file")]
        path: String,
        #[arg(long, help = "Contract name")]
        id: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum LocalnetCommand {
    #[command(about = "Start the local TON network")]
    Start {
        #[arg(long, help = "Localnet server port (default: [localnet].port or 5411)")]
        port: Option<u16>,
        #[arg(
            long,
            help = "Fork from network for remote account resolution (default: [localnet].fork-net)"
        )]
        fork_net: Option<String>,
        #[arg(
            long,
            help = "Block sequence number to fork from (default: [localnet].fork-block-number)",
            value_name = "SEQNO"
        )]
        fork_block_number: Option<u64>,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Wallet names to auto-fund and deploy on startup (default: [localnet].accounts)",
            value_name = "NAME[,NAME...]"
        )]
        accounts: Option<Vec<String>>,
        #[arg(long, help = "Path to SQLite database for persistent storage")]
        db_path: Option<String>,
        #[arg(
            long,
            value_name = "RPS",
            value_parser = clap::value_parser!(u32).range(1..),
            help = "Maximum API requests per second to simulate provider rate limits (default: [localnet].rate-limit)"
        )]
        rate_limit: Option<u32>,
        #[arg(
            long,
            help = "Load Localnet state from JSON snapshot before startup",
            conflicts_with = "db_path", // for now
            value_name = "PATH"
        )]
        load_state: Option<String>,
        #[arg(
            long,
            help = "Dump Localnet state to JSON snapshot on shutdown",
            value_name = "PATH"
        )]
        dump_state: Option<String>,
    },
    #[command(about = "Request TON from faucet")]
    Airdrop {
        #[arg(help = "Address to receive TON")]
        address: String,
        #[arg(long, short, help = "Amount of TON to request", default_value = "100")]
        amount: f64,
        #[arg(
            long,
            short,
            help = "Localnet server port (default: [localnet].port or 5411)"
        )]
        port: Option<u16>,
    },
}

#[derive(Subcommand, Clone)]
pub enum LibraryCommand {
    #[command(about = "Publish a library to the blockchain")]
    Publish {
        #[arg(help = "Contract name to publish (see --code to pass arbitrary code)", value_name = "CONTRACT_NAME", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Code to use instead of compiling contract")]
        code: Option<String>,
        #[arg(
            long,
            help = "Duration to publish the library for (e.g. 100d, 1y); prompts if not provided"
        )]
        duration: Option<String>,
        #[arg(long, help = "Wallet to use for publishing (prompts if not provided)", add = ArgValueCompleter::new(complete_wallets))]
        wallet: Option<String>,
        #[arg(long, help = "Network to use", default_value = "testnet")]
        net: String,
        #[arg(long, help = "Amount of TON to send for publication")]
        amount: Option<String>,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
        #[arg(long, help = "Save library info to local libraries.toml")]
        local: bool,
        #[arg(long, help = "Save library info to global.libraries.toml")]
        global: bool,
    },
    #[command(about = "Fetch a library from the blockchain")]
    Fetch {
        #[arg(help = "Library hash to fetch")]
        hash: String,
        #[arg(long, help = "Disassemble fetched library code")]
        disasm: bool,
        #[arg(
            long,
            short,
            help = "Output file for fetched code (BoC, base64 or TASM if --disasm provided)"
        )]
        output: Option<String>,
        #[arg(long, help = "Network to use", default_value = "testnet")]
        net: String,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "Display information about a deployed library")]
    Info {
        #[arg(help = "Library name to show info for")]
        name: Option<String>,
    },
    #[command(about = "Top up a library's account for storage")]
    Topup {
        #[arg(help = "Library name to top up", value_name = "LIBRARY_NAME")]
        name: Option<String>,
        #[arg(
            long,
            help = "Duration to top up for (e.g. 100d, 1y); prompts if not provided"
        )]
        duration: Option<String>,
        #[arg(long, help = "Wallet to use for topping up (prompts if not provided)")]
        wallet: Option<String>,
        #[arg(
            long,
            help = "Amount of TON to send (overrides duration-based calculation)"
        )]
        amount: Option<String>,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum DocCommand {
    #[command(about = "Lookup an instruction in the TVM specification")]
    Tvm {
        #[arg(
            help = "Instruction name(s) or search query (for example: ADD SENDRAWMSG)",
            num_args = 1..
        )]
        instruction: Vec<String>,
        #[arg(short = 'f', long, help = "Find instructions by fuzzy query")]
        find: bool,
        #[arg(
            short = 'd',
            long,
            requires = "find",
            help = "Include instruction descriptions in fuzzy search"
        )]
        description: bool,
        #[arg(long, help = "Output instruction entry as JSON")]
        json: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum MetaCommand {
    #[command(about = "Print a built-in JSON schema")]
    GetSchema {
        #[arg(value_enum, default_value = "acton-toml", help = "Schema to print")]
        schema: BuiltinSchema,
    },
}

#[inline]
const fn get_acton_version() -> &'static str {
    acton::build_info::LONG_VERSION
}

fn complete_contracts(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(config) = load_config_for_completion() else {
        return vec![];
    };

    let current = current.to_string_lossy();
    config
        .contracts
        .unwrap_or_default()
        .contracts
        .keys()
        .filter(|contract| contract.starts_with(current.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

fn complete_contracts_or_paths(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut candidates = complete_contracts(current);
    candidates.extend(PathCompleter::any().complete(current));
    candidates
}

fn complete_scripts(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(config) = load_config_for_completion() else {
        return vec![];
    };

    let current = current.to_string_lossy();
    config
        .scripts
        .unwrap_or_default()
        .keys()
        .filter(|script| script.starts_with(current.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

fn complete_wallets(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let current = current.to_string_lossy();
    let mut wallets = std::collections::BTreeMap::new();

    let load_wallets = |path: &Path| -> Option<std::collections::BTreeMap<String, _>> {
        let content = fs::read_to_string(path).ok()?;
        let file: WalletsFile = toml::from_str(&content).ok()?;
        Some(file.wallets?.wallets)
    };

    // 1. Global wallets
    if let Some(global_path) = global_wallets_path()
        && let Some(w) = load_wallets(&global_path)
    {
        wallets.extend(w);
    }

    // 2. Local wallets.toml (overrides global)
    if let Some(project_root) = find_project_root_for_completion()
        && let Some(w) = load_wallets(&project_root.join("wallets.toml"))
    {
        wallets.extend(w);
    }

    wallets
        .keys()
        .filter(|name| name.starts_with(current.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

fn complete_commands(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let current = current.to_string_lossy();
    base_cli_command()
        .get_subcommands()
        .filter(|cmd| !cmd.is_hide_set())
        .map(|cmd| cmd.get_name().to_string())
        .filter(|name| name.starts_with(current.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

fn find_project_root_for_completion() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let manifest = find_manifest_in_ancestors(&cwd)?;
    manifest.parent().map(Path::to_path_buf)
}

fn load_config_for_completion() -> Option<ActonConfig> {
    let cwd = env::current_dir().ok()?;
    let manifest = find_manifest_in_ancestors(&cwd)?;
    let content = fs::read_to_string(manifest).ok()?;
    toml::from_str::<ActonConfig>(&content).ok()
}

fn detailed_help_pointer(command: &str) -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styles = acton_help_styles();
    let literal = styles.get_literal();
    let _ = write!(
        writer,
        "Run '{literal}acton help {command}{literal:#}' for more detailed information."
    );
    writer
}

const fn acton_help_styles() -> Styles {
    let header = Style::new().bold();
    let usage = Style::new().bold();
    let literal = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
        .bold();
    let placeholder = Style::new().dimmed();

    Styles::styled()
        .header(header)
        .usage(usage)
        .literal(literal)
        .placeholder(placeholder)
        .context(placeholder)
        .context_value(placeholder)
}

fn root_help(show_global_options: bool) -> StyledStr {
    use std::collections::HashMap;
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let header = Style::new().bold();
    let usage_style = Style::new().bold();
    let dimmed = Style::new().dimmed();
    let purple = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
        .bold();
    let blue = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::Blue)))
        .bold();
    let yellow = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
        .bold();
    let cyan = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
        .bold();
    let white = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::BrightWhite)))
        .bold();

    let core_commands = vec![("new", "[PATH]"), ("init", "")];
    let build_and_test_commands = vec![
        ("test", "[PATH]"),
        ("build", "[CONTRACT_NAME]"),
        ("check", "[TARGET]"),
        ("script", "<PATH> [ARGS...]"),
        ("fmt", "[PATHS...]"),
    ];
    let blockchain_commands = vec![
        ("wallet", "<COMMAND>"),
        ("rpc", "<COMMAND>"),
        ("verify", "[CONTRACT_NAME]"),
        ("library", "<COMMAND>"),
        // ("localnet", "<COMMAND>"),
        ("retrace", "<TX_HASH>"),
    ];
    let tooling_commands = vec![
        ("run", "<SCRIPT> [ARGS...]"),
        ("compile", "<PATH>"),
        ("wrapper", "<CONTRACT_NAME>"),
        ("disasm", "[BOC_FILE]"),
        ("doc", "tvm <QUERY...>"),
    ];
    let support_commands = vec![
        // ("ls", ""),
        ("up", ""),
        ("help", "[COMMAND]"),
        ("hooks", "<COMMAND>"),
        ("doctor", ""),
        ("func2tolk", "<PATH>"),
        ("completions", "<SHELL>"),
    ];

    let command_groups = [
        (&purple, core_commands),
        (&blue, build_and_test_commands),
        (&yellow, blockchain_commands),
        (&cyan, tooling_commands),
        (&white, support_commands),
    ];
    let mut command_metadata = base_cli_command();
    command_metadata.build();

    let command_descriptions = command_metadata
        .get_subcommands()
        .map(|subcommand| {
            (
                subcommand.get_name().to_owned(),
                subcommand
                    .get_about()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            )
        })
        .collect::<HashMap<_, _>>();

    let all_entries = command_groups
        .iter()
        .flat_map(|(_, entries)| entries)
        .collect::<Vec<_>>();

    let max_name = all_entries
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    let max_hint = all_entries
        .iter()
        .map(|(_, hint)| hint.len())
        .max()
        .unwrap_or(0);
    let global_options = command_metadata
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .filter(|arg| !arg.is_positional())
        .filter(|arg| arg.is_global_set() || matches!(arg.get_long(), Some("help" | "version")))
        .filter_map(|arg| {
            let name = if matches!(arg.get_long(), Some("version")) {
                "-v, --version".to_owned()
            } else {
                match (arg.get_short(), arg.get_long()) {
                    (Some(short), Some(long)) => format!("-{short}, --{long}"),
                    (Some(short), None) => format!("-{short}"),
                    (None, Some(long)) => format!("--{long}"),
                    (None, None) => return None,
                }
            };

            let hint = arg
                .get_value_names()
                .and_then(|value_names| value_names.first())
                .map(|value_name| format!("<{value_name}>"))
                .unwrap_or_default();

            let description = arg.get_help().map(ToString::to_string).unwrap_or_default();
            if description.is_empty() {
                return None;
            }

            Some((name, hint, description))
        })
        .collect::<Vec<_>>();
    let max_option_name = global_options
        .iter()
        .map(|(name, _, _)| name.len())
        .max()
        .unwrap_or(0);
    let max_option_hint = global_options
        .iter()
        .map(|(_, hint, _)| hint.len())
        .max()
        .unwrap_or(0);
    let align_name = max_name.max(max_option_name);
    let align_hint = max_hint.max(max_option_hint);

    let _ = write!(
        writer,
        "{purple}Acton{purple:#} is all-in-one on-chain development tool for TON.",
    );
    let _ = write!(
        writer,
        "\n\n{header}Usage:{header:#} {usage_style}acton <command> [...flags] [...args]{usage_style:#}"
    );

    let _ = write!(writer, "\n\n{header}Commands:{header:#}");
    for (group_idx, (command_style, entries)) in command_groups.iter().enumerate() {
        if group_idx > 0 {
            let _ = writeln!(writer);
        }
        for (name, hint) in entries {
            let description = command_descriptions
                .get(*name)
                .map(String::as_str)
                .unwrap_or_default();
            let _ = write!(
                writer,
                "\n  {command_style}{name:<align_name$}{command_style:#}  "
            );
            if hint.is_empty() {
                let _ = write!(writer, "{:align_hint$}  ", "", align_hint = align_hint);
            } else {
                let _ = write!(writer, "{dimmed}{hint:<align_hint$}{dimmed:#}  ");
            }
            let _ = write!(writer, "{description}");
        }
    }

    if show_global_options {
        let _ = write!(writer, "\n\n{header}Global options:{header:#}");
        for (name, hint, description) in &global_options {
            let _ = write!(writer, "\n  {cyan}{name:<align_name$}{cyan:#}  ",);
            if hint.is_empty() {
                let _ = write!(writer, "{:align_hint$}  ", "", align_hint = align_hint);
            } else {
                let _ = write!(writer, "{dimmed}{hint:<align_hint$}{dimmed:#}  ",);
            }
            let _ = write!(writer, "{description}");
        }
    }

    let _ = write!(
        writer,
        "\n\nUse {cyan}acton help <command>{cyan:#} for detailed manuals with behavior, config, and examples."
    );

    let _ = writeln!(
        writer,
        "\n\nLearn more about Acton:                {cyan}https://ton-blockchain.github.io/acton/docs/welcome{cyan:#}"
    );

    writer
}

fn base_cli_command() -> clap::Command {
    Cli::command()
        .styles(acton_help_styles())
        .disable_version_flag(true)
        .arg(
            clap::Arg::new("version")
                .short('v')
                .short_alias('V')
                .long("version")
                .action(ArgAction::Version)
                .help("Print version"),
        )
}

fn cli_command(show_global_options: bool) -> clap::Command {
    base_cli_command().override_help(root_help(show_global_options))
}

fn completion_command() -> clap::Command {
    cli_command(true)
}

fn root_help_has_explicit_help_flag() -> bool {
    env::args_os()
        .skip(1)
        .any(|arg| arg == "-h" || arg == "--help")
}

fn render_help_command(command: Option<String>) -> anyhow::Result<()> {
    match command.as_deref() {
        None => {
            cli_command(true).print_help()?;
            println!();
            Ok(())
        }
        Some(command) => {
            if print_command_manual(command)? {
                return Ok(());
            }

            let mut cli = base_cli_command();
            cli.build();
            if let Some(subcommand) = cli.find_subcommand_mut(command) {
                subcommand.print_long_help()?;
                println!();
                return Ok(());
            }

            let mut message = format!("no such command: `{command}`");
            if let Some((suggestion, _)) = cli
                .get_subcommands()
                .map(clap::Command::get_name)
                .filter(|name| *name != "help")
                .map(|name| (name, strsim::jaro_winkler(command, name)))
                .filter(|(_, score)| *score >= 0.80)
                .max_by(|left, right| left.1.total_cmp(&right.1))
            {
                let _ = write!(
                    message,
                    "\n\nhelp: a command with a similar name exists: `{suggestion}`"
                );
            }
            message.push_str("\n\nhelp: view all commands with `acton --help`");

            base_cli_command()
                .error(clap::error::ErrorKind::InvalidSubcommand, message)
                .exit();
        }
    }
}

fn find_manifest_in_ancestors(start_dir: &Path) -> Option<PathBuf> {
    let git_boundary = start_dir
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf);

    start_dir
        .ancestors()
        .take_while(|dir| match &git_boundary {
            Some(boundary) => dir.starts_with(boundary),
            None => true,
        })
        .find_map(|dir| {
            let candidate = dir.join("Acton.toml");
            candidate.is_file().then_some(candidate)
        })
}

struct ResolvedProjectRoot {
    path: PathBuf,
    source: ResolutionSource,
}

struct ResolvedManifestPath {
    path: PathBuf,
    source: ResolutionSource,
}

fn resolve_manifest_path(
    manifest_path: Option<PathBuf>,
    resolved_project_root: &ResolvedProjectRoot,
) -> anyhow::Result<ResolvedManifestPath> {
    let cwd = env::current_dir()?;

    if let Some(manifest_path) = manifest_path {
        let mut resolved = if manifest_path.is_absolute() {
            manifest_path
        } else {
            cwd.join(manifest_path)
        };

        if resolved.is_dir() {
            resolved = resolved.join("Acton.toml");
        }

        return Ok(ResolvedManifestPath {
            path: resolved,
            source: ResolutionSource::ManifestPathFlag,
        });
    }

    let source = match resolved_project_root.source {
        ResolutionSource::ProjectRootFlag => ResolutionSource::ProjectRootFlag,
        ResolutionSource::AutoDetected => ResolutionSource::AutoDetected,
        ResolutionSource::FallbackCwd | ResolutionSource::ManifestPathFlag => {
            ResolutionSource::FallbackCwd
        }
    };

    Ok(ResolvedManifestPath {
        path: resolved_project_root.path.join("Acton.toml"),
        source,
    })
}

fn resolve_project_root(project_root: Option<PathBuf>) -> anyhow::Result<ResolvedProjectRoot> {
    let cwd = env::current_dir()?;

    if let Some(project_root) = project_root {
        let resolved_project_root = if project_root.is_absolute() {
            project_root
        } else {
            cwd.join(project_root)
        };

        if !resolved_project_root.is_dir() {
            anyhow::bail!(
                "Project root {} is not a directory",
                resolved_project_root.display()
            );
        }

        return Ok(ResolvedProjectRoot {
            path: resolved_project_root,
            source: ResolutionSource::ProjectRootFlag,
        });
    }

    if let Some(found_manifest_path) = find_manifest_in_ancestors(&cwd)
        && let Some(parent) = found_manifest_path.parent()
    {
        return Ok(ResolvedProjectRoot {
            path: parent.to_path_buf(),
            source: ResolutionSource::AutoDetected,
        });
    }

    Ok(ResolvedProjectRoot {
        path: cwd,
        source: ResolutionSource::FallbackCwd,
    })
}

fn configure_project_roots(
    manifest_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
) -> anyhow::Result<()> {
    let resolved_project_root = resolve_project_root(project_root)?;
    let resolved_manifest_path = resolve_manifest_path(manifest_path, &resolved_project_root)?;

    init_project_root_with_source(&resolved_project_root.path, resolved_project_root.source)?;
    init_manifest_path_with_source(&resolved_manifest_path.path, resolved_manifest_path.source)?;

    Ok(())
}

fn main() {
    CompleteEnv::with_factory(completion_command).complete();

    setup_panic!(
        Metadata::new("Acton", acton::build_info::SHORT_VERSION)
            .authors("TON Core")
            .homepage("https://github.com/ton-blockchain/acton")
    );
    let _crash_handler = acton::crash::install().map_err(|err| {
        eprintln!(
            "Warning: failed to install fatal signal handler ({err}). Continuing without fatal signal diagnostics."
        );
        err
    }).ok();

    dotenv().ok();
    let Cli {
        color,
        manifest_path,
        project_root,
        command,
    } = {
        let matches = cli_command(root_help_has_explicit_help_flag()).get_matches();
        Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
    };
    init_color_mode(color);

    if command_configures_project_roots(&command) {
        if let Err(err) = configure_project_roots(manifest_path.clone(), project_root.clone()) {
            eprintln!("{} {}", "Error:".red(), err);
            process::exit(1);
        }

        if command_checks_toolchain_version(&command)
            && let Err(err) = validate_project_toolchain_version()
        {
            print_error(&err);
            process::exit(1);
        }
    }

    if !matches!(
        command,
        Commands::Ls { .. } | Commands::Help { .. } | Commands::Meta { .. } | Commands::Lint { .. }
    ) && let Err(_) = setup_logging()
    {
        // previously we print error here, but it is too annoying for LLM agents
        // we need some better way
    }

    let result = match command {
        Commands::Init {
            create_dapp,
            stdlib_only,
        } => init_cmd(create_dapp.as_deref(), stdlib_only),
        Commands::Help { command } => render_help_command(command),
        Commands::Wallet { command } => wallet_cmd(command),
        Commands::Rpc { command } => {
            if manifest_path.is_some() || project_root.is_some() {
                match configure_project_roots(manifest_path, project_root) {
                    Ok(()) => validate_project_toolchain_version().and_then(|()| rpc_cmd(command)),
                    Err(err) => Err(err),
                }
            } else {
                rpc_cmd(command)
            }
        }
        Commands::New {
            path,
            name,
            description,
            template,
            license,
            app,
            hooks,
            agents,
            templates,
        } => new_cmd(
            path.as_deref(),
            name,
            description,
            template,
            license,
            app,
            hooks,
            agents,
            templates,
        ),
        Commands::Test {
            path,
            filter,
            reporter,
            show_bodies,
            verbose,
            debug,
            debug_port,
            backtrace,
            coverage,
            coverage_format,
            coverage_file,
            coverage_minimum_percent,
            coverage_include_wrappers,
            coverage_include_tests,
            exclude,
            include,
            clear_cache,
            junit_path,
            junit_merge,
            snapshot,
            baseline_snapshot,
            fail_on_diff,
            fork_net,
            save_test_trace,
            mutate,
            mutate_overrides,
            mutate_contract,
            mutation_rules_file,
            mutation_session_id,
            mutation_workers,
            mutation_diff,
            mutation_diff_ref,
            mutation_levels,
            id,
            mutation_minimum_percent,
            mutation_disable_rules,
            fail_fast,
            fuzz_seed,
            fork_block_number,
            ui,
            ui_port,
        } => match (
            fork_net.as_deref().map(Network::from_str).transpose(),
            commands::common::validate_cli_verbosity(verbose),
        ) {
            (Ok(fork_net), Ok(verbose)) => {
                match create_test_config(
                    filter,
                    show_bodies,
                    verbose,
                    debug,
                    debug_port,
                    backtrace,
                    coverage,
                    coverage_format,
                    coverage_file,
                    coverage_minimum_percent,
                    coverage_include_wrappers,
                    coverage_include_tests,
                    exclude,
                    include,
                    clear_cache,
                    reporter,
                    junit_path,
                    junit_merge,
                    snapshot,
                    baseline_snapshot,
                    fail_on_diff,
                    fork_net,
                    fork_block_number,
                    save_test_trace.or_else(|| {
                        if ui {
                            Some(paths::DEFAULT_BUILD_TRACES_DIR.to_owned())
                        } else {
                            None
                        }
                    }),
                    mutate,
                    mutate_overrides,
                    mutate_contract,
                    mutation_rules_file,
                    mutation_session_id,
                    mutation_workers,
                    mutation_diff,
                    mutation_diff_ref,
                    mutation_levels,
                    id,
                    mutation_minimum_percent,
                    mutation_disable_rules,
                    fuzz_seed,
                    fail_fast,
                    ui,
                    ui_port,
                ) {
                    Ok(config) => {
                        if mutate {
                            mutation::test_mutate_cmd(path.as_deref(), &config)
                        } else {
                            test_cmd(path, &config)
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            (Err(err), _) | (_, Err(err)) => Err(err),
        },
        Commands::Run { script, args } => run_cmd(&script, &args),
        Commands::Retrace {
            hash,
            net,
            verbose,
            logs_dir,
            contract,
            debug,
            debug_port,
        } => retrace_cmd(hash, net, verbose, logs_dir, contract, debug, debug_port),
        Commands::Wrapper {
            contract_id,
            output: wrapper_output,
            output_dir: wrapper_output_dir,
            test_output,
            test_output_dir,
            test,
            ts,
        } => wrapper_cmd(
            &contract_id,
            wrapper_output,
            wrapper_output_dir,
            test_output,
            test_output_dir,
            test,
            ts,
        ),
        Commands::Script {
            path,
            args,
            verbose,
            debug,
            backtrace,
            debug_port,
            clear_cache,
            fork_net,
            fork_block_number,
            net,
            tonconnect,
            tonconnect_port,
            explorer,
            show_bodies,
        } => match commands::common::validate_cli_verbosity(verbose) {
            Ok(verbose) => script_cmd(
                &path,
                args,
                verbose,
                debug,
                backtrace,
                debug_port,
                clear_cache,
                fork_net,
                fork_block_number,
                net,
                explorer,
                show_bodies,
                tonconnect,
                tonconnect_port,
            ),
            Err(err) => Err(err),
        },
        Commands::Build {
            contract_id,
            clear_cache,
            graph,
            out_dir,
            gen_dir,
            output_abi,
            output_fift,
            info,
        } => build_cmd(BuildCommandOptions {
            contract_id,
            clear_cache,
            graph_output: graph,
            out_dir,
            gen_dir,
            output_abi,
            output_fift,
            show_info: info,
        }),
        Commands::Compile {
            path,
            json,
            base64_only,
            boc,
            fift,
            source_map,
            abi,
            allow_no_entrypoint,
            clear_cache,
        } => {
            let result = compile_cmd(
                &path,
                json,
                base64_only,
                boc,
                fift,
                source_map,
                abi,
                allow_no_entrypoint,
                clear_cache,
            );
            if json {
                if let Err(err) = result {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "success": false,
                            "error": err.to_string()
                        }))
                        .expect("JSON serialization should not fail")
                    );
                    process::exit(1);
                }
                return;
            }
            result
        }
        Commands::Disasm {
            boc_file,
            string,
            output,
            show_hashes,
            show_offsets,
            json,
            source_map,
            address,
            net,
            follow_libraries,
        } => {
            let result = match read_source_map(source_map) {
                Ok(source_map) => disasm_cmd(
                    boc_file,
                    string,
                    output,
                    FormatOptions {
                        show_hashes,
                        show_offsets,
                        source_map,
                    },
                    address,
                    net,
                    follow_libraries,
                    json,
                ),
                Err(err) => Err(err),
            };
            if json {
                report_error_as_json(result);
                return;
            }
            result
        }
        Commands::Verify {
            contract_id,
            address,
            net,
            wallet,
            compiler_version,
            dry_run,
        } => verify_cmd(contract_id, address, net, wallet, compiler_version, dry_run),
        Commands::Library { command } => match command {
            LibraryCommand::Publish {
                contract_id,
                code,
                duration,
                wallet,
                net,
                amount,
                yes,
                local,
                global,
            } => publish_cmd(
                contract_id,
                code,
                duration,
                wallet,
                net,
                amount,
                yes,
                local,
                global,
            ),
            LibraryCommand::Fetch {
                hash,
                disasm,
                output,
                net,
                json,
            } => {
                let result = fetch_cmd(hash, disasm, output, net, json);
                if json {
                    report_error_as_json(result);
                    return;
                }
                result
            }
            LibraryCommand::Info { name } => info_cmd(name),
            LibraryCommand::Topup {
                name,
                duration,
                wallet,
                amount,
                yes,
            } => commands::library::topup_cmd(name, duration, wallet, amount, yes),
        },
        Commands::Check {
            fix,
            output_format,
            output_file,
            enable_only,
            explain,
            list_lint_rules,
            target,
        } => check_cmd(
            fix,
            output_format,
            output_file,
            enable_only,
            explain,
            list_lint_rules,
            target,
        ),
        Commands::Lint { args } => Err(lint_command_error(&args)),
        Commands::Up {
            version,
            trunk,
            stable,
            force,
            list,
            check,
        } => {
            let result = up_cmd(version, trunk, stable, force, list, check);
            if check {
                report_error_as_json(result);
                return;
            }
            result
        }
        Commands::Fmt { paths, check } => fmt_cmd(paths, check),
        Commands::Doc { command } => match command {
            DocCommand::Tvm {
                instruction,
                find,
                description,
                json,
            } => doc_tvm_cmd(&instruction, json, find, description),
        },
        Commands::Func2Tolk {
            path,
            output,
            warnings_as_comments,
            no_camel_case,
            version,
        } => func2tolk_cmd(path, output, warnings_as_comments, no_camel_case, version),
        Commands::Hooks { command } => hooks_cmd(command),
        Commands::Doctor => doctor_cmd(),
        Commands::Completions { shell } => {
            if shell == "nushell" {
                clap_complete::generate(
                    clap_complete_nushell::Nushell,
                    &mut base_cli_command(),
                    "acton",
                    &mut std::io::stdout(),
                );
            } else {
                let shell = clap_complete::Shell::from_str(&shell)
                    .expect("validated completion shell should parse");
                clap_complete::generate(
                    shell,
                    &mut base_cli_command(),
                    "acton",
                    &mut std::io::stdout(),
                );
            }
            Ok(())
        }
        Commands::Meta { command } => match command {
            MetaCommand::GetSchema { schema } => print_schema_cmd(schema),
        },
        Commands::Docgen { output, check } => docgen_cmd(output, check),
        Commands::Ls {
            port,
            stdio,
            log_file,
            no_log,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize tokio runtime for language server");
            rt.block_on(ls_cmd(port, stdio, log_file, no_log))
        }
        Commands::InternalRegisterContract { path, id } => internal_register_contract(&path, id),
        Commands::Localnet { command } => match command {
            LocalnetCommand::Start {
                port,
                fork_net,
                fork_block_number,
                accounts,
                db_path,
                rate_limit,
                load_state,
                dump_state,
            } => {
                let resolved_localnet = resolve_localnet_settings(
                    port,
                    fork_net,
                    fork_block_number,
                    accounts,
                    rate_limit,
                );
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");
                rt.block_on(async {
                    commands::localnet::localnet_start_cmd(
                        resolved_localnet.port,
                        db_path,
                        resolved_localnet.fork_net,
                        resolved_localnet.fork_block_number,
                        resolved_localnet.accounts,
                        resolved_localnet.rate_limit,
                        load_state,
                        dump_state,
                    )
                    .await
                })
            }
            LocalnetCommand::Airdrop {
                address,
                amount,
                port,
            } => {
                let port = resolve_localnet_port(port);
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");
                rt.block_on(async {
                    commands::localnet::localnet_airdrop_cmd(&address, amount, port).await
                })
            }
        },
    };

    if let Err(err) = result {
        print_error(&err);
        process::exit(1)
    }
}

const fn command_configures_project_roots(command: &Commands) -> bool {
    !matches!(
        command,
        Commands::Init { .. }
            | Commands::New { .. }
            | Commands::Help { .. }
            | Commands::Rpc { .. }
            | Commands::Meta { .. }
            | Commands::Lint { .. }
    )
}

const fn command_checks_toolchain_version(command: &Commands) -> bool {
    command_configures_project_roots(command)
        && !matches!(command, Commands::Up { .. } | Commands::Completions { .. })
}

fn validate_project_toolchain_version() -> anyhow::Result<()> {
    if !configured_manifest_path().exists() {
        return Ok(());
    }

    let config = ActonConfig::load_manifest()?;
    let Some(expected) = config
        .toolchain
        .as_ref()
        .and_then(|toolchain| toolchain.acton.as_deref())
    else {
        return Ok(());
    };

    let expected = expected.trim();
    if expected.is_empty() {
        anyhow::bail!(
            "Acton.toml has empty [toolchain].acton.\n\nSet it to the required Acton CLI version, for example:\n\n[toolchain]\nacton = \"{}\"",
            acton::build_info::SHORT_VERSION
        );
    }

    let installed = acton::build_info::SHORT_VERSION;
    if expected == installed {
        return Ok(());
    }

    anyhow::bail!(
        "Acton CLI version mismatch for this project.\n\nActon.toml expects [toolchain].acton = \"{expected}\"\nInstalled acton version is \"{installed}\".\n\nInstall the expected version:\n  acton up {expected}\n\nOr update [toolchain].acton if this project supports acton {installed}."
    );
}

fn print_error(err: &anyhow::Error) {
    eprintln!("{} {}", "Error:".red(), err);

    for cause in err.chain().skip(1) {
        eprintln!("\nCaused by:");
        for line in cause.to_string().lines() {
            eprintln!("  {line}");
        }
    }
}

fn lint_command_error(args: &[String]) -> anyhow::Error {
    let suffix = if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    };

    anyhow::anyhow!("`acton lint` is not supported. Use `acton check{suffix}` instead.")
}

struct ResolvedLocalnetSettings {
    port: u16,
    fork_net: Option<String>,
    fork_block_number: Option<u64>,
    accounts: Vec<String>,
    rate_limit: Option<u32>,
}

fn resolve_localnet_port(cli_port: Option<u16>) -> u16 {
    resolve_localnet_settings(cli_port, None, None, None, None).port
}

fn resolve_localnet_settings(
    cli_port: Option<u16>,
    cli_fork_net: Option<String>,
    cli_fork_block_number: Option<u64>,
    cli_accounts: Option<Vec<String>>,
    cli_rate_limit: Option<u32>,
) -> ResolvedLocalnetSettings {
    let config = load_localnet_settings_from_config();
    ResolvedLocalnetSettings {
        port: cli_port.or(config.port).unwrap_or(5411),
        fork_net: cli_fork_net.or(config.fork_net),
        fork_block_number: cli_fork_block_number.or(config.fork_block_number),
        accounts: cli_accounts.or(config.accounts).unwrap_or_default(),
        rate_limit: cli_rate_limit.or(config.rate_limit),
    }
}

fn load_localnet_settings_from_config() -> LocalnetSettings {
    ActonConfig::load()
        .ok()
        .and_then(|config| config.localnet)
        .unwrap_or_default()
}

fn report_error_as_json<T>(result: anyhow::Result<T>) {
    if let Err(err) = result {
        println!(
            "{}",
            serde_json::json!({
                "success": false,
                "error": err.to_string()
            })
        );
    }
}

fn read_source_map(source_map: Option<String>) -> anyhow::Result<Option<Box<SourceMap>>> {
    let source_map_data = if let Some(path) = source_map {
        if !fs::exists(&path).unwrap_or(false) {
            anyhow::bail!(error_fmt::file_not_found(&path));
        }

        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            anyhow::bail!("{} is not a file", path.yellow());
        }

        let content = fs::read_to_string(&path)
            .map_err(|err| anyhow::anyhow!("Cannot access {}: {err}", path.yellow()))?;
        let result = serde_json::from_str::<SourceMap>(content.as_str()).map_err(|err| {
            anyhow::anyhow!("Failed to parse source map {}: {err}", path.yellow())
        })?;
        Some(Box::new(result))
    } else {
        None
    };
    Ok(source_map_data)
}

const ACTON_LOG_DIR_ENV: &str = "ACTON_LOG_DIR";

fn env_path(var: &str) -> Option<PathBuf> {
    let value = env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn resolve_acton_log_dir_with_env(
    mut get_env_path: impl FnMut(&str) -> Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = get_env_path(ACTON_LOG_DIR_ENV) {
        return path;
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = get_env_path("USERPROFILE") {
            return path.join(".acton").join("logs");
        }
        if let Some(path) = get_env_path("HOME") {
            return path.join(".acton").join("logs");
        }
        return paths::build_logs_dir(configured_project_root());
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = get_env_path("HOME") {
            return path.join(".acton").join("logs");
        }
        return paths::build_logs_dir(configured_project_root());
    }

    #[allow(unreachable_code)]
    paths::build_logs_dir(configured_project_root())
}

fn resolve_acton_log_dir() -> PathBuf {
    resolve_acton_log_dir_with_env(env_path)
}

fn setup_logging() -> anyhow::Result<()> {
    let log_dir = resolve_acton_log_dir();
    fs::create_dir_all(&log_dir)?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("debug.log"))?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                message
            ));
        })
        .level(log::LevelFilter::Debug)
        .chain(log_file)
        .apply()?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_test_config(
    filter: Option<String>,
    show_bodies: bool,
    verbosity: u8,
    debug: bool,
    debug_port: Option<u16>,
    backtrace: Option<BacktraceMode>,
    coverage: bool,
    coverage_format: Option<CoverageFormat>,
    coverage_file: Option<String>,
    coverage_minimum_percent: Option<f64>,
    coverage_include_wrappers: bool,
    coverage_include_tests: bool,
    exclude: Vec<String>,
    include: Vec<String>,
    clear_cache: Option<bool>,
    report_formats: Vec<ReportFormat>,
    junit_path: Option<String>,
    junit_merge: bool,
    snapshot: Option<String>,
    baseline_snapshot: Option<String>,
    fail_on_diff: bool,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    save_test_trace: Option<String>,
    mutate: bool,
    mutate_overrides: Option<String>,
    mutate_contract: Option<String>,
    mutation_rules_file: Option<String>,
    mutation_session_id: Option<String>,
    mutation_workers: Option<usize>,
    mutation_diff: Option<MutationDiffMode>,
    mutation_diff_ref: Option<String>,
    mutation_levels: Vec<MutationLevel>,
    mutation_ids: Vec<usize>,
    mutation_minimum_percent: Option<f64>,
    disable_rules: Vec<String>,
    fuzz_seed: Option<u64>,
    fail_fast: Option<bool>,
    ui: bool,
    ui_port: Option<u16>,
) -> anyhow::Result<TestConfig> {
    let acton_config = ActonConfig::load();

    if let Ok(acton_config) = &acton_config
        && let Some(test_settings) = &acton_config.test
    {
        validate_test_settings(test_settings)?;

        let mut config = test_settings.to_test_config(
            filter,
            report_formats,
            show_bodies,
            if debug { Some(true) } else { None },
            debug_port,
            backtrace,
            if coverage { Some(true) } else { None },
            coverage_format,
            coverage_file,
            coverage_minimum_percent,
            if coverage_include_wrappers {
                Some(true)
            } else {
                None
            },
            if coverage_include_tests {
                Some(true)
            } else {
                None
            },
            if exclude.is_empty() {
                None
            } else {
                Some(exclude)
            },
            if include.is_empty() {
                None
            } else {
                Some(include)
            },
            clear_cache,
            junit_path,
            junit_merge,
            snapshot,
            baseline_snapshot,
            fork_net,
            fork_block_number,
            save_test_trace,
            mutate,
            mutate_overrides,
            mutate_contract,
            mutation_diff,
            mutation_diff_ref,
            mutation_levels,
            mutation_minimum_percent,
            disable_rules,
            fuzz_seed,
            if fail_on_diff { Some(true) } else { None },
            fail_fast,
            ui,
            ui_port,
        );
        config.verbosity = verbosity;
        config.mutation_ids = mutation_ids;
        if mutation_rules_file.is_some() {
            config.mutation_rules_file = mutation_rules_file;
        }
        config.mutation_session_id = mutation_session_id;
        config.mutation_workers = mutation_workers;
        validate_merged_test_fork_network(Some(acton_config), config.fork_net.as_ref())?;
        return Ok(config);
    }

    let config = TestConfig {
        show_bodies,
        verbosity,
        debug,
        debug_port: debug_port.unwrap_or(12345),
        backtrace,
        coverage,
        coverage_minimum_percent,
        coverage_include_wrappers,
        coverage_include_tests,
        filter,
        coverage_format,
        coverage_file,
        exclude_patterns: exclude,
        include_patterns: include,
        clear_cache: clear_cache.unwrap_or(false),
        report_formats,
        junit_path,
        junit_merge,
        snapshot,
        baseline_snapshot,
        fail_on_diff,
        fork_block_number,
        save_test_trace,
        mutate,
        mutate_overrides,
        mutate_contract,
        mutation_rules_file,
        mutation_session_id,
        mutation_workers,
        mutation_diff,
        mutation_diff_ref,
        mutation_levels,
        mutation_ids,
        mutation_minimum_percent,
        disable_rules,
        fuzz_runs: None,
        fuzz_max_test_rejects: None,
        fuzz_seed,
        fail_fast: fail_fast.unwrap_or(false),
        ui,
        ui_port: ui_port.unwrap_or(12344),
        fork_net,
    };

    validate_merged_test_fork_network(acton_config.as_ref().ok(), config.fork_net.as_ref())?;

    Ok(config)
}

fn validate_test_settings(test_settings: &TestSettings) -> anyhow::Result<()> {
    if let Some(fork_net) = test_settings.fork_net.as_deref() {
        Network::from_str(fork_net)
            .map_err(|err| anyhow::anyhow!("Invalid [test].fork-net '{fork_net}': {err}"))?;
    }

    Ok(())
}

fn validate_merged_test_fork_network(
    acton_config: Option<&ActonConfig>,
    fork_net: Option<&Network>,
) -> anyhow::Result<()> {
    let Some(fork_net) = fork_net else {
        return Ok(());
    };

    let Some(acton_config) = acton_config else {
        if let Network::Custom(name) = fork_net {
            anyhow::bail!(
                "Custom test fork network 'custom:{name}' requires Acton.toml with [networks.{name}.api].v2"
            );
        }
        return Ok(());
    };

    if let Network::Custom(name) = fork_net {
        validate_custom_test_network(acton_config, name)?;
    }

    let custom_networks = acton_config.custom_networks();
    let v2_url = fork_net
        .toncenter_v2_url(&custom_networks)
        .map_err(|err| anyhow::anyhow!("Invalid test fork network '{fork_net}': {err}"))?;
    reqwest::Url::parse(&v2_url).map_err(|err| {
        anyhow::anyhow!("Invalid TonCenter v2 URL for test fork network '{fork_net}': {err}")
    })?;

    Ok(())
}

fn validate_custom_test_network(acton_config: &ActonConfig, name: &str) -> anyhow::Result<()> {
    let network = acton_config
        .networks
        .as_ref()
        .and_then(|networks| networks.get(name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown custom test fork network 'custom:{name}'. Define [networks.{name}.api].v2 in Acton.toml."
            )
        })?;

    let has_v2 = network
        .api
        .as_ref()
        .and_then(|api| api.v2.as_deref())
        .is_some_and(|url| !url.trim().is_empty());
    if !has_v2 {
        anyhow::bail!(
            "Custom test fork network 'custom:{name}' must define [networks.{name}.api].v2 in Acton.toml."
        );
    }

    Ok(())
}

fn parse_coverage_percent(raw: &str) -> Result<f64, String> {
    parse_minimum_percent(raw, "coverage percentage", "--coverage-minimum-percent")
}

fn parse_mutation_percent(raw: &str) -> Result<f64, String> {
    parse_minimum_percent(raw, "mutation percentage", "--mutation-minimum-percent")
}

fn parse_mutation_id(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|err| format!("invalid mutation ID '{raw}': {err}"))?;

    if value == 0 {
        return Err("--mutation-id must be 1 or greater".to_string());
    }

    Ok(value)
}

fn parse_mutation_workers(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|err| format!("invalid mutation worker count '{raw}': {err}"))?;

    if value == 0 {
        return Err("--mutation-workers must be 1 or greater".to_string());
    }

    Ok(value)
}

fn parse_minimum_percent(raw: &str, kind: &str, flag: &str) -> Result<f64, String> {
    let value = raw
        .parse::<f64>()
        .map_err(|err| format!("invalid {kind} '{raw}': {err}"))?;

    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(format!("{flag} must be between 0 and 100"));
    }

    Ok(value)
}
