use acton::commands;
use acton::commands::build::build_cmd;
use acton::commands::check::check_cmd;
use acton::commands::compile::compile_cmd;
use acton::commands::disasm::disasm_cmd;
use acton::commands::docgen::docgen_cmd;
use acton::commands::fmt::fmt_cmd;
use acton::commands::init::init_cmd;
use acton::commands::internal::internal_register_contract;
use acton::commands::library::{fetch_cmd, info_cmd, publish_cmd};
use acton::commands::ls::ls_cmd;
use acton::commands::new::new_cmd;
use acton::commands::retrace::retrace_cmd;
use acton::commands::run::run_cmd;
use acton::commands::script::script_cmd;
use acton::commands::test::{mutation, test_cmd};
use acton::commands::up::up_cmd;
use acton::commands::verify::verify_cmd;
use acton::commands::wallet::{WalletCommand, wallet_cmd};
use acton::commands::wrapper::wrapper_cmd;
use acton_config::color::OwoColorize;
use acton_config::color::{ColorMode, init_color_mode};
use acton_config::config::{ActonConfig, Explorer, Network, init_manifest_path};
use acton_config::test::{BacktraceMode, CoverageFormat, ReportFormat, TestConfig};
use clap::builder::styling::Style;
use clap::builder::{StyledStr, Styles};
use clap::{ColorChoice, CommandFactory};
use clap::{Parser, Subcommand};
use clap_complete::CompleteEnv;
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use commands::common::error_fmt;
use dotenvy::dotenv;
use human_panic::{Metadata, setup_panic};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::str::FromStr;
use std::{env, fs, process};
use tasm::printer::FormatOptions;
use ton_source_map::SourceMap;

#[derive(Parser)]
#[command(
    name = "acton",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")")
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

    #[command(subcommand)]
    command: Commands,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Initialize a new project in the current directory",
        long_about = "Initialize a new project in the current directory. This is useful for adding Acton support to an existing project."
    )]
    Init,
    #[command(
        about = "Create a new project in a specified directory",
        long_about = "Create a new project in a specified directory. This will create a new directory with a basic project template.",
        after_help = example_new_usage()
    )]
    New {
        #[arg(help = "Directory to create the project in (use '.' for the current directory)")]
        path: String,
        #[arg(long, help = "Project name")]
        name: Option<String>,
        #[arg(long, help = "Project description")]
        description: Option<String>,
        #[arg(long, help = "Project template")]
        template: Option<String>,
        #[arg(long, help = "License")]
        license: Option<String>,
    },
    #[command(
        about = "Manage wallets",
        after_help = example_wallet_usage()
    )]
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    #[command(
        about = "Execute tests in file or directory",
        after_help = example_test_usage()
    )]
    Test {
        #[arg(help = "Test file or directory containing test files (default: current directory)")]
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
            help = "Stop executing tests after the first failure",
            help_heading = "Execution"
        )]
        fail_fast: bool,

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
            default_value = "test-results",
            help = "JUnit XML output directory",
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
        #[arg(
            long,
            help = "TonCenter API key for blockchain queries",
            help_heading = "Remote"
        )]
        api_key: Option<String>,

        // Tracing
        #[arg(
            long,
            help = "Save transaction traces to directory",
            help_heading = "Tracing",
            value_name = "DIR",
            default_missing_value = ".acton/traces",
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
            value_name = "CONTRACT_ID"
        )]
        mutate_contract: Option<String>,
        #[arg(
            long,
            help = "Disable specific mutation rules",
            help_heading = "Mutation Testing",
            value_name = "RULE"
        )]
        disable_rule: Vec<String>,
        #[arg(
            long,
            help = "Open test results in a browser",
            help_heading = "Reporting"
        )]
        ui: bool,
        #[arg(
            long,
            help = "UI server port",
            default_value = "12344",
            help_heading = "Reporting",
            value_name = "PORT"
        )]
        ui_port: u16,
    },
    #[command(
        about = "Generate wrapper and optionally stub test file for a contract",
        after_help = example_wrapper_usage()
    )]
    Wrapper {
        #[arg(help = "Contract ID to generate wrapper", value_name = "CONTRACT_ID", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: String,
        #[arg(long, short, help = "Output path for wrapper file")]
        output: Option<String>,

        #[arg(
            long,
            short,
            help = "Generate a stub test file for contract",
            default_value = "false",
            help_heading = "Tests"
        )]
        test: bool,
        #[arg(long, help = "Output path for test file", help_heading = "Tests")]
        test_output: Option<String>,
        #[arg(long, help = "Storage struct name to use for wrapper generation")]
        storage_struct: Option<String>,
    },
    #[command(
        about = "Execute a Tolk script file",
        after_help = example_script_usage()
    )]
    Script {
        #[arg(help = "Script file to execute")]
        path: String,

        #[arg(help = "Arguments to pass to the script")]
        args: Vec<String>,

        // Debugging
        #[arg(long, help = "Enable debug mode", help_heading = "Debugging")]
        debug: bool,
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
        #[arg(
            long,
            help = "TonCenter API key for blockchain queries",
            help_heading = "Remote"
        )]
        api_key: Option<String>,

        // Broadcasting
        #[arg(
            long,
            help = "Send transactions to the blockchain instead of emulating them",
            help_heading = "Broadcasting"
        )]
        broadcast: bool,

        #[arg(
            long,
            help = "Network to use for broadcasting",
            help_heading = "Broadcasting"
        )]
        net: Option<String>,

        #[arg(
            value_enum,
            long,
            help = "Explorer to use for transaction links",
            help_heading = "Broadcasting",
            value_name = "NAME"
        )]
        explorer: Option<Explorer>,
    },
    #[command(
        about = "Build the specified contract or all contracts",
        after_help = example_build_usage()
    )]
    Build {
        #[arg(help = "Contract ID to build (defaults to all if not specified)", value_name = "CONTRACT_ID", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Clear compilation cache before building")]
        clear_cache: bool,
        #[arg(
            long,
            help = "Generate dependency graph as SVG file (requires graphviz)"
        )]
        graph: Option<String>,
        #[arg(
            long,
            default_value = "build",
            help = "Output directory for build artifacts"
        )]
        out_dir: Option<String>,
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
        about = "Run a script defined in Acton.toml",
        after_help = example_run_usage()
    )]
    Run {
        #[arg(help = "Name of the script to run", add = ArgValueCompleter::new(complete_scripts))]
        script: String,
        #[arg(
            help = "Arguments to pass to the script",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    #[command(
        about = "Compile a Tolk file",
        after_help = example_compile_usage()
    )]
    Compile {
        #[arg(help = "Tolk file to compile")]
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
        #[arg(long, help = "Clear compilation cache before running")]
        clear_cache: bool,
    },
    #[command(
        about = "Disassemble TVM bitcode to human-readable TASM",
        after_help = example_disasm_usage()
    )]
    Disasm {
        #[arg(help = "Binary/Hex/Base64 BoC file to disassemble (use -s flag to pass a string)")]
        boc_file: Option<String>,
        #[arg(short, long, help = "BoC string in hex or base64 format")]
        string: Option<String>,
        #[arg(short, long, help = "Output file (if not specified, output to stdout)")]
        output: Option<String>,
        #[arg(long, help = "Show cell hashes and offsets for each cell")]
        show_hashes: bool,
        #[arg(long, help = "Show instruction offsets in left column")]
        show_offsets: bool,
        #[arg(long, help = "Source map file for showing Tolk source locations")]
        source_map: Option<String>,
        #[arg(
            long,
            help = "Contract address to fetch from blockchain (e.g., UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM)"
        )]
        address: Option<String>,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
        #[arg(long, help = "Network to use for fetching from blockchain")]
        net: Option<String>,
        #[arg(
            long,
            help = "Follow library references and disassemble the actual library code instead of showing library hash"
        )]
        follow_libraries: bool,
    },
    #[command(
        about = "Verify contract source code on verifier.ton.org",
        after_help = example_verify_usage()
    )]
    Verify {
        #[arg(help = "Contract ID to verify (prompts if not provided)", value_name = "CONTRACT_ID", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Deployed contract address (prompts if not provided)")]
        address: Option<String>,
        #[arg(long, help = "Network to use", default_value = "testnet")]
        net: String,
        #[arg(
            long,
            help = "Wallet from Acton.toml to use for verification (defaults to the only one if single wallet configured)"
        )]
        wallet: Option<String>,
        #[arg(long, help = "Tolk compiler version to use on verifier side")]
        compiler_version: Option<String>,
        #[arg(long, help = "Run verification without sending the final transaction")]
        dry_run: bool,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
    },
    #[command(about = "Check Tolk files in the project for errors")]
    Check {
        #[arg(help = "Contract ID to check or path to a .tolk file")]
        target: Option<String>,
        #[arg(long, help = "Automatically apply available fixes")]
        fix: bool,
        #[arg(long, help = "Output results as JSON")]
        json: bool,
        #[arg(long, help = "Explain a rule")]
        explain: Option<String>,
        #[arg(long, hide = true)]
        list_lint_rules: bool,
    },
    #[command(
        about = "Retrace a transaction by its hash",
        after_help = example_retrace_usage()
    )]
    Retrace {
        #[arg(help = "Transaction hash in hex format to retrace")]
        hash: String,
        #[arg(long, help = "Network to use")]
        net: Option<String>,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
        #[arg(
            short,
            long,
            help = "Show full cell hex instead of hashes in out actions"
        )]
        verbose: bool,
        #[arg(long, help = "Directory to save VM and executor logs")]
        logs_dir: Option<String>,
    },
    #[command(
        about = "Manage TON libraries",
        after_help = example_library_usage()
    )]
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
    #[command(
        about = "Manage lightweight TON node",
        after_help = example_litenode_usage()
    )]
    Litenode {
        #[command(subcommand)]
        command: LitenodeCommand,
    },
    #[command(
        about = "Format Tolk source files",
        after_help = example_fmt_usage()
    )]
    Fmt {
        #[arg(help = "Files or directories to format (defaults to current directory)")]
        paths: Vec<String>,
        #[arg(long, help = "Check if files are formatted without overwriting them")]
        check: bool,
    },
    #[command(about = "LSP server for the TON languages and technologies")]
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
        about = "Manage Acton versions",
        after_help = example_up_usage()
    )]
    Up {
        #[arg(help = "Specific version to install")]
        version: Option<String>,
        #[arg(long, help = "Install the most recent canary release")]
        canary: bool,
        #[arg(long, help = "Install the latest stable release")]
        stable: bool,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
        #[arg(long, help = "List available versions")]
        list: bool,
        #[arg(long, hide = true, help = "Check for updates and return info as JSON")]
        check: bool,
    },
    #[command(
        about = "Generate shell completions for selected shell",
        after_help = example_completions_usage()
    )]
    Completions {
        #[clap(value_enum)]
        shell: clap_complete::Shell,
    },
    #[command(
        about = "Internal command to generate MDX documentation from standard library",
        hide = true
    )]
    Docgen {
        #[arg(short, long, help = "Output directory path")]
        output: Option<String>,
    },
    #[command(name = "internal-register-contract", hide = true)]
    InternalRegisterContract {
        #[arg(help = "Path to the contract file")]
        path: String,
        #[arg(long, help = "Contract ID")]
        id: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum LitenodeCommand {
    #[command(about = "Start the lightweight TON node")]
    Start {
        #[arg(long, default_value_t = 3000)]
        port: u16,
        #[arg(long, help = "Fork from network for remote account resolution")]
        fork_net: Option<String>,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
        #[arg(long, help = "Path to SQLite database for persistent storage")]
        db_path: Option<String>,
    },
    #[command(about = "Request TON from faucet")]
    Airdrop {
        #[arg(help = "Address to receive TON")]
        address: String,
        #[arg(long, short, help = "Amount of TON to request", default_value = "100")]
        amount: f64,
        #[arg(long, short, help = "LiteNode server port", default_value_t = 3000)]
        port: u16,
    },
}

#[derive(Subcommand, Clone)]
pub enum LibraryCommand {
    #[command(about = "Publish a library to the blockchain")]
    Publish {
        #[arg(help = "Contract ID to publish (see --code to pass arbitrary code)", value_name = "CONTRACT_ID", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Code to use instead of compiling contract")]
        code: Option<String>,
        #[arg(
            long,
            help = "Duration to publish the library for (e.g. 100d, 1y); prompts if not provided"
        )]
        duration: Option<String>,
        #[arg(long, help = "Wallet to use for publishing (prompts if not provided)")]
        wallet: Option<String>,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
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
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
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
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
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
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
        #[arg(
            long,
            help = "Amount of TON to send (overrides duration-based calculation)"
        )]
        amount: Option<String>,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
    },
}

fn example_litenode_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Start the lightweight TON node on default port 3000",
                "acton litenode start",
            ),
            (
                "Request 100 TON from faucet to specified address",
                "acton litenode airdrop UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM",
            ),
            (
                "Request specific amount of TON from faucet",
                "acton litenode airdrop UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM --amount 50",
            ),
        ],
        "",
    )
}

fn example_test_usage() -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    let exampled_command = Vec::from([
        ("Run all tests in current directory", "acton test"),
        ("Run tests in specific file", "acton test my_test.tolk"),
        (
            "Run tests in directory with regex filter",
            "acton test . --filter \"wallet.*\"",
        ),
        (
            "Exclude tests",
            "acton test . --exclude \"**/integration/**\"",
        ),
        (
            "Exclude multiple patterns",
            "acton test . --exclude \"**/e2e/**\" --exclude \"**/gas/**\"",
        ),
        (
            "Include only specific directories",
            "acton test . --include \"**/unit/**\" --include \"**/wallet/**\"",
        ),
        (
            "Enable coverage collection",
            "acton test . --coverage --coverage-format lcov",
        ),
        (
            "Run with teamcity service messages",
            "acton test . --reporter console,teamcity",
        ),
        (
            "Generate JUnit XML report",
            "acton test . --reporter junit --junit-path ./test-results",
        ),
        (
            "Generate merged JUnit XML report",
            "acton test . --reporter junit --junit-merge",
        ),
        ("Run in debug mode", "acton test my_test.tolk --debug"),
    ]);

    let header = styled.get_header();
    let named = Style::new().dimmed();
    let example = styled.get_literal();

    let _ = write!(writer, "{header}Examples:{header:#}",);

    const USAGE_SEP: &str = "\n     ";
    for (name, value) in &exampled_command {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{example}{value}{example:#}");
    }

    let _ = write!(
        writer,
        "\nFor more information, see https://i582.github.io/acton/docs/test-runner"
    );

    writer
}

fn complete_contracts(_current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Ok(config) = ActonConfig::load() else {
        return vec![];
    };

    config
        .contracts
        .unwrap_or_default()
        .contracts
        .keys()
        .map(CompletionCandidate::new)
        .collect()
}

fn complete_scripts(_current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Ok(config) = ActonConfig::load() else {
        return vec![];
    };

    config
        .scripts
        .unwrap_or_default()
        .keys()
        .map(CompletionCandidate::new)
        .collect()
}

fn example_build_usage() -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    let dim = |text: &str| text.dimmed().to_string();
    let green = |text: &str| text.green().to_string();

    let config_example = [
        format!("{}contracts.wallet{}", dim("["), dim("]")),
        format!("     name{}{}", dim(" = "), green("\"Wallet Contract\"")),
        format!(
            "     src{}{}",
            dim(" = "),
            green("\"contracts/wallet.tolk\"")
        ),
        format!("     output{}{}", dim(" = "), green("\"wallet.boc\"")),
        format!(
            "     depends{}{}{}{}",
            dim(" = "),
            dim("["),
            green("\"child\""),
            dim("]")
        ),
        format!(
            "     {}",
            dim("# or as library with custom function name and output path")
        ),
        format!("     depends{}{}", dim(" = "), dim("[")),
        format!(
            "       {} name{}{}{} kind{}{}{} function{}{}{} path{}{} {}",
            dim("{"),
            dim(" = "),
            green("\"child\""),
            dim(","),
            dim(" = "),
            green("\"library_ref\""),
            dim(","),
            dim(" = "),
            green("\"getChildCode\""),
            dim(","),
            dim(" = "),
            green("\"child_dep.tolk\""),
            dim("}")
        ),
        format!("     {}", dim("]")),
    ]
    .join("\n");

    let build_config_example = [
        format!("{}build{}", dim("["), dim("]")),
        format!("     output-fift{}{}", dim(" = "), green("\"build/fift\"")),
    ]
    .join("\n");

    let build_examples = Vec::from([
        ("Build all contracts", "acton build"),
        ("Build specific contract", "acton build wallet"),
        (
            "Build contracts with fresh cache",
            "acton build --clear-cache",
        ),
        (
            "Generate dependency graph as SVG file",
            "acton build --graph deps.svg",
        ),
        (
            "Save compiled Fift files to a custom directory",
            "acton build --output-fift build/fift",
        ),
    ]);

    let header = styled.get_header();
    let named = Style::new().dimmed();
    let literal = styled.get_literal();

    let _ = write!(writer, "{header}Configuration:{header:#}");
    let _ = write!(
        writer,
        "\n     {named}# Configure contracts in Acton.toml{named:#}"
    );
    let _ = write!(writer, "\n     {config_example}");
    let _ = write!(
        writer,
        "\n\n     {named}# Optional build output settings{named:#}"
    );
    let _ = write!(writer, "\n     {build_config_example}");
    let _ = write!(writer, "\n\n{header}Examples:{header:#}");

    const USAGE_SEP: &str = "\n     ";
    for (name, value) in &build_examples {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{literal}{value}{literal:#}");
    }

    let _ = write!(
        writer,
        "\nFor more information, see https://i582.github.io/acton/docs/build-system"
    );

    writer
}

fn example_disasm_usage() -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    let disasm_examples = Vec::from([
        ("Disassemble from BoC file", "acton disasm contract.boc"),
        (
            "Disassemble from hex/base64 string",
            "acton disasm -s \"b5ee9c72010104...0840f01c700f2f4\"",
        ),
        (
            "Disassemble from blockchain address",
            "acton disasm --address UQA...wwM",
        ),
        (
            "Disassemble with output to file",
            "acton disasm contract.boc -o output.tasm",
        ),
        (
            "Disassemble with cell hashes and offsets",
            "acton disasm contract.boc --show-hashes --show-offsets",
        ),
        (
            "Disassemble from testnet address",
            "acton disasm --address kQAl...g44 --net testnet",
        ),
    ]);

    let header = styled.get_header();
    let named = Style::new().dimmed();
    let literal = styled.get_literal();

    let _ = write!(writer, "{header}Examples:{header:#}");

    const USAGE_SEP: &str = "\n     ";
    for (name, value) in &disasm_examples {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{literal}{value}{literal:#}");
    }

    let _ = write!(
        writer,
        "\nFor more information, see https://i582.github.io/acton/docs/commands/disasm"
    );

    writer
}

fn format_examples(examples: &[(&str, &str)], link: &str) -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    let header = styled.get_header();
    let named = Style::new().dimmed();
    let literal = styled.get_literal();

    let _ = write!(writer, "{header}Examples:{header:#}");

    const USAGE_SEP: &str = "\n     ";
    for (name, value) in examples {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{literal}{value}{literal:#}");
    }

    if !link.is_empty() {
        let _ = write!(writer, "\nFor more information, see {link}");
    }

    writer
}

fn example_new_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Create a new project named my-project",
                "acton new my-project",
            ),
            (
                "Create a project non-interactively with all metadata",
                "acton new my-project --name \"My Project\" --description \"Cool description\" --template counter --license MIT",
            ),
        ],
        "https://i582.github.io/acton/docs/commands/new",
    )
}

fn example_wallet_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Create a new wallet named my-wallet",
                "acton wallet new my-wallet",
            ),
            (
                "List all configured wallets with balances",
                "acton wallet list -b",
            ),
            (
                "Request testnet TONs from faucet",
                "acton wallet airdrop my-wallet",
            ),
        ],
        "https://i582.github.io/acton/docs/commands/wallet",
    )
}

fn example_wrapper_usage() -> StyledStr {
    format_examples(
        &[
            ("Generate wrapper for minter", "acton wrapper minter"),
            (
                "Generate wrapper and stub test for minter",
                "acton wrapper minter --test",
            ),
        ],
        "https://i582.github.io/acton/docs/test-runner/generating-wrappers",
    )
}

fn example_script_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Execute a deploy script in local emulator",
                "acton script scripts/deploy.tolk",
            ),
            (
                "Execute a deploy script and broadcast to testnet network",
                "acton script scripts/deploy.tolk --net testnet",
            ),
            (
                "Execute a deploy script and broadcast to mainnet network",
                "acton script scripts/deploy.tolk --net mainnet",
            ),
        ],
        "https://i582.github.io/acton/docs/scripting",
    )
}

fn example_run_usage() -> StyledStr {
    format_examples(
        &[(
            "Run a custom script named 'deploy' with arguments",
            "acton run deploy 1 2 3",
        )],
        "https://i582.github.io/acton/docs/commands/run",
    )
}

fn example_compile_usage() -> StyledStr {
    format_examples(
        &[(
            "Compile a Tolk contract and save as BOC",
            "acton compile contracts/main.tolk --boc main.boc",
        )],
        "https://i582.github.io/acton/docs/commands/compile",
    )
}

fn example_verify_usage() -> StyledStr {
    format_examples(
        &[(
            "Verify a contract with a specific address",
            "acton verify minter --address UQA...wwM",
        )],
        "https://i582.github.io/acton/docs/contract-verification",
    )
}

fn example_retrace_usage() -> StyledStr {
    format_examples(
        &[(
            "Retrace a transaction by its hash",
            "acton retrace 287f...9e0",
        )],
        "",
    )
}

fn example_library_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Publish a contract as a library",
                "acton library publish minter",
            ),
            ("Fetch a library by its hash", "acton library fetch <HASH>"),
            (
                "Show information about a library",
                "acton library info my-lib",
            ),
            (
                "Top up a library for 1 year",
                "acton library topup my-lib --duration 1y",
            ),
        ],
        "https://i582.github.io/acton/docs/advanced/libraries",
    )
}

fn example_up_usage() -> StyledStr {
    format_examples(
        &[
            ("Upgrade Acton to the latest stable version", "acton up"),
            ("List all available versions", "acton up --list"),
        ],
        "https://i582.github.io/acton/docs/installation",
    )
}

fn example_fmt_usage() -> StyledStr {
    format_examples(
        &[
            ("Format all Tolk files in the current project", "acton fmt"),
            (
                "Format specific files or directories",
                "acton fmt contracts/ scripts/",
            ),
            ("Check if all files are formatted", "acton fmt --check"),
        ],
        "",
    )
}

fn example_completions_usage() -> StyledStr {
    format_examples(
        &[
            (
                "Generate dynamic Bash completions",
                "source <(COMPLETE=bash acton)",
            ),
            ("Generate static Zsh completions", "acton completions zsh"),
        ],
        "https://i582.github.io/acton/docs/commands/shell-completions",
    )
}

fn configure_manifest_path(manifest_path: Option<PathBuf>) -> anyhow::Result<()> {
    let is_custom_manifest = manifest_path.is_some();
    let manifest_path = manifest_path.unwrap_or_else(|| PathBuf::from("Acton.toml"));
    let mut resolved_manifest_path = if manifest_path.is_absolute() {
        manifest_path
    } else {
        env::current_dir()?.join(manifest_path)
    };

    if resolved_manifest_path.is_dir() {
        resolved_manifest_path = resolved_manifest_path.join("Acton.toml");
    }

    init_manifest_path(&resolved_manifest_path)?;

    // Keep relative paths in commands stable by working from the manifest directory.
    if is_custom_manifest
        && let Some(project_dir) = resolved_manifest_path.parent()
        && project_dir.exists()
    {
        env::set_current_dir(project_dir)?;
    }

    Ok(())
}

fn main() {
    CompleteEnv::with_factory(Cli::command).complete();

    setup_panic!(
        Metadata::new("Acton", env!("CARGO_PKG_VERSION"))
            .authors("TON Core")
            .homepage("https://github.com/i582/acton")
    );
    dotenv().ok();
    let Cli {
        color,
        manifest_path,
        command,
    } = Cli::parse();
    init_color_mode(color);

    if let Err(err) = configure_manifest_path(manifest_path) {
        eprintln!("{} {}", "Error:".red(), err);
        process::exit(1);
    }

    if !matches!(command, Commands::Ls { .. }) {
        // for language server we set up own logging
        setup_logging().expect("Failed to set up logging");
    }

    let result = match command {
        Commands::Init => init_cmd(),
        Commands::Wallet { command } => wallet_cmd(command),
        Commands::New {
            path,
            name,
            description,
            template,
            license,
        } => new_cmd(&path, name, description, template, license),
        Commands::Test {
            path,
            filter,
            reporter,
            debug,
            debug_port,
            backtrace,
            coverage,
            coverage_format,
            coverage_file,
            exclude,
            include,
            clear_cache,
            junit_path,
            junit_merge,
            snapshot,
            baseline_snapshot,
            fork_net,
            api_key,
            save_test_trace,
            mutate,
            mutate_overrides,
            mutate_contract,
            disable_rule,
            fail_fast,
            fork_block_number,
            ui,
            ui_port,
        } => match fork_net.as_deref().map(Network::from_str).transpose() {
            Ok(fork_net) => {
                let config = create_test_config(
                    filter,
                    debug,
                    debug_port,
                    backtrace,
                    coverage,
                    coverage_format,
                    coverage_file,
                    exclude,
                    include,
                    clear_cache,
                    reporter,
                    junit_path,
                    junit_merge,
                    snapshot,
                    baseline_snapshot,
                    fork_net,
                    api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                    fork_block_number,
                    save_test_trace.or_else(|| {
                        if ui {
                            Some(".acton/traces".to_owned())
                        } else {
                            None
                        }
                    }),
                    mutate,
                    mutate_overrides,
                    mutate_contract,
                    disable_rule,
                    Some(fail_fast),
                    ui,
                    ui_port,
                );

                if mutate {
                    mutation::test_mutate_cmd(&path, &config)
                } else {
                    test_cmd(path, &config)
                }
            }
            Err(err) => Err(err),
        },
        Commands::Run { script, args } => run_cmd(&script, &args),
        Commands::Retrace {
            hash,
            net,
            api_key,
            verbose,
            logs_dir,
        } => retrace_cmd(hash, net, api_key, verbose, logs_dir),
        Commands::Wrapper {
            contract_id,
            output: wrapper_output,
            test_output,
            test,
            storage_struct,
        } => wrapper_cmd(
            &contract_id,
            wrapper_output,
            test_output,
            test,
            storage_struct,
        ),
        Commands::Script {
            path,
            args,
            debug,
            debug_port,
            clear_cache,
            fork_net,
            api_key,
            fork_block_number,
            broadcast,
            net,
            explorer,
        } => script_cmd(
            &path,
            args,
            debug,
            debug_port,
            clear_cache,
            fork_net,
            api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
            fork_block_number,
            broadcast,
            net,
            explorer,
        ),
        Commands::Build {
            contract_id,
            clear_cache,
            graph,
            out_dir,
            output_fift,
            info,
        } => build_cmd(contract_id, clear_cache, graph, out_dir, output_fift, info),
        Commands::Compile {
            path,
            json,
            base64_only,
            boc,
            fift,
            source_map,
            abi,
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
            source_map,
            address,
            api_key,
            net,
            follow_libraries,
        } => match read_source_map(source_map) {
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
                api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                net,
                follow_libraries,
            ),
            Err(err) => Err(err),
        },
        Commands::Verify {
            contract_id,
            address,
            net,
            wallet,
            compiler_version,
            dry_run,
            api_key,
        } => verify_cmd(
            contract_id,
            address,
            net,
            wallet,
            compiler_version,
            dry_run,
            api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
        ),
        Commands::Library { command } => match command {
            LibraryCommand::Publish {
                contract_id,
                code,
                duration,
                wallet,
                api_key,
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
                api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                net,
                amount,
                yes,
                local,
                global,
            ),
            LibraryCommand::Fetch {
                hash,
                disasm,
                api_key,
                output,
                net,
                json,
            } => {
                let result = fetch_cmd(
                    hash,
                    disasm,
                    api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                    output,
                    net,
                    json,
                );
                if json {
                    report_error_as_json(result);
                    return;
                }
                result
            }
            LibraryCommand::Info { name, api_key } => {
                info_cmd(name, api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()))
            }
            LibraryCommand::Topup {
                name,
                duration,
                wallet,
                api_key,
                amount,
                yes,
            } => commands::library::topup_cmd(
                name,
                duration,
                wallet,
                api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                amount,
                yes,
            ),
        },
        Commands::Check {
            target,
            fix,
            json,
            explain,
            list_lint_rules,
        } => check_cmd(fix, json, explain, list_lint_rules, target),
        Commands::Up {
            version,
            canary,
            stable,
            yes,
            list,
            check,
        } => {
            let result = up_cmd(version, canary, stable, yes, list, check);
            if check {
                report_error_as_json(result);
                return;
            }
            result
        }
        Commands::Fmt { paths, check } => fmt_cmd(paths, check),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "acton", &mut std::io::stdout());
            Ok(())
        }
        Commands::Docgen { output } => docgen_cmd(output),
        Commands::Ls {
            port,
            stdio,
            log_file,
            no_log,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize tokio runtime for langauge server");
            rt.block_on(ls_cmd(port, stdio, log_file, no_log))
        }
        Commands::InternalRegisterContract { path, id } => internal_register_contract(&path, id),
        Commands::Litenode { command } => match command {
            LitenodeCommand::Start {
                port,
                fork_net,
                api_key,
                db_path,
            } => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");
                rt.block_on(async {
                    commands::litenode::litenode_start_cmd(
                        port,
                        db_path,
                        fork_net,
                        api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                    )
                    .await
                })
            }
            LitenodeCommand::Airdrop {
                address,
                amount,
                port,
            } => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");
                rt.block_on(async {
                    commands::litenode::litenode_airdrop_cmd(&address, amount, port).await
                })
            }
        },
    };

    if let Err(err) = result {
        eprintln!("{} {}", "Error:".red(), err);
        process::exit(1)
    }
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

        let content = fs::read_to_string(path).expect("Failed to read source map file");
        let result: SourceMap =
            serde_json::from_str(content.as_str()).expect("Failed to parse source map JSON");
        Some(Box::new(result))
    } else {
        None
    };
    Ok(source_map_data)
}

fn setup_logging() -> anyhow::Result<()> {
    fs::create_dir_all(".acton/")?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(".acton/debug.log")?;

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
        // .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_test_config(
    filter: Option<String>,
    debug: bool,
    debug_port: Option<u16>,
    backtrace: Option<BacktraceMode>,
    coverage: bool,
    coverage_format: Option<CoverageFormat>,
    coverage_file: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    clear_cache: bool,
    report_formats: Vec<ReportFormat>,
    junit_path: Option<String>,
    junit_merge: bool,
    snapshot: Option<String>,
    baseline_snapshot: Option<String>,
    fork_net: Option<Network>,
    api_key: Option<String>,
    fork_block_number: Option<u64>,
    save_test_trace: Option<String>,
    mutate: bool,
    mutate_overrides: Option<String>,
    mutate_contract: Option<String>,
    disable_rules: Vec<String>,
    fail_fast: Option<bool>,
    ui: bool,
    ui_port: u16,
) -> TestConfig {
    let acton_config = ActonConfig::load();

    if let Ok(acton_config) = acton_config
        && let Some(test_settings) = &acton_config.test
    {
        return test_settings.to_test_config(
            filter,
            report_formats,
            if debug { Some(true) } else { None },
            debug_port,
            backtrace,
            if coverage { Some(true) } else { None },
            coverage_format,
            coverage_file,
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
            None,
            junit_path,
            junit_merge,
            snapshot,
            baseline_snapshot,
            fork_net,
            api_key,
            fork_block_number,
            save_test_trace,
            mutate,
            mutate_overrides,
            mutate_contract,
            disable_rules,
            fail_fast,
            ui,
            Some(ui_port),
        );
    }

    TestConfig {
        debug,
        debug_port: debug_port.unwrap_or(12345),
        backtrace,
        coverage,
        filter,
        coverage_format,
        coverage_file,
        exclude_patterns: exclude,
        include_patterns: include,
        clear_cache,
        report_formats,
        junit_path,
        junit_merge,
        snapshot,
        baseline_snapshot,
        api_key,
        fork_block_number,
        save_test_trace,
        mutate,
        mutate_overrides,
        mutate_contract,
        disable_rules,
        fail_fast: fail_fast.unwrap_or(false),
        ui,
        ui_port,
        fork_net,
    }
}
