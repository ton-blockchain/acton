use acton::commands;
use acton::commands::build::build_cmd;
use acton::commands::compile::compile_cmd;
use acton::commands::disasm::disasm_cmd;
use acton::commands::docgen::docgen_cmd;
use acton::commands::init::init_cmd;
use acton::commands::library::{fetch_cmd, publish_cmd};
use acton::commands::new::new_cmd;
use acton::commands::retrace::retrace_cmd;
use acton::commands::run::run_cmd;
use acton::commands::script::script_cmd;
use acton::commands::test::{
    BacktraceMode, CoverageFormat, ReportFormat, TestConfig, mutation, test_cmd,
};
use acton::commands::up::up_cmd;
use acton::commands::verify::verify_cmd;
use acton::commands::wallet::{WalletCommand, wallet_cmd};
use acton::commands::wrapper::wrapper_cmd;
use acton::config::{ActonConfig, Explorer, Network};
use clap::builder::styling::Style;
use clap::builder::{StyledStr, Styles};
use clap::{ColorChoice, CommandFactory};
use clap::{Parser, Subcommand, arg};
use clap_complete::CompleteEnv;
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use commands::common::error_fmt;
use dotenvy::dotenv;
use human_panic::{Metadata, setup_panic};
use owo_colors::OwoColorize;
use std::fs::OpenOptions;
use std::{env, fs, process};
use tasm::printer::FormatOptions;
use tolkc::source_map::SourceMap;

#[derive(Parser)]
#[command(
    name = "acton",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")")
)]
#[command(about = "TON blockchain development tool")]
#[command(color = ColorChoice::Auto)]
struct Cli {
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
        long_about = "Create a new project in a specified directory. This will create a new directory with a basic project template."
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
    #[command(about = "Manage wallets")]
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
        #[arg(
            long,
            help = "Debug server port",
            default_value = "12345",
            help_heading = "Debugging"
        )]
        debug_port: u16,
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
        fork_net: Option<Network>,
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
    },
    #[command(about = "Generate wrapper and optionally stub test file for a contract")]
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
    #[command(about = "Execute a Tolk script file")]
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
        fork_net: Option<Network>,
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
        net: Option<Network>,

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
        #[arg(long, help = "Show compiled contract info")]
        info: bool,
    },
    #[command(about = "Run a script defined in Acton.toml")]
    Run {
        #[arg(help = "Name of the script to run")]
        script: String,
        #[arg(
            help = "Arguments to pass to the script",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    #[command(about = "Compile a Tolk file")]
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
        #[arg(
            long,
            help = "Network to use for fetching from blockchain",
            default_value = "mainnet"
        )]
        net: Network,
        #[arg(
            long,
            help = "Follow library references and disassemble the actual library code instead of showing library hash"
        )]
        follow_libraries: bool,
    },
    #[command(about = "Verify contract source code on verifier.ton.org")]
    Verify {
        #[arg(help = "Contract ID to verify (prompts if not provided)", value_name = "CONTRACT_ID", add = ArgValueCompleter::new(complete_contracts))]
        contract_id: Option<String>,
        #[arg(long, help = "Deployed contract address (prompts if not provided)")]
        address: Option<String>,
        #[arg(long, help = "Network to use", default_value = "testnet")]
        net: Network,
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
    #[command(about = "Retrace a transaction by its hash")]
    Retrace {
        #[arg(help = "Transaction hash in hex format to retrace")]
        hash: String,
        #[arg(long, help = "Network to use")]
        net: Option<Network>,
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
    #[command(about = "Manage TON libraries")]
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
    #[command(about = "Manage Acton versions")]
    Up {
        #[arg(help = "Specific version to install")]
        version: Option<String>,
        #[arg(long, help = "Install the most recent canary release")]
        canary: bool,
        #[arg(long, help = "Install the latest stable release")]
        stable: bool,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
    },
    #[command(
        about = "Generate shell completions for selected shell",
        after_help = "For installation instructions, see https://acton.dev/acton/shell-completions/"
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
        net: Network,
        #[arg(long, help = "Amount of TON to send for publication")]
        amount: Option<f64>,
        #[arg(short, long, help = "Skip confirmation prompts")]
        yes: bool,
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
        net: Network,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
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
    for (name, value) in exampled_command.iter() {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{example}{value}{example:#}");
    }

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

fn example_build_usage() -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    // for some reason `cstr` cannot output `{` correctly :/
    let config_example = color_print::cformat!(
        r#"<dim>[</>contracts.wallet<dim>]</>
     name<dim> = </><green>"Wallet Contract"</>
     src<dim> = </><green>"contracts/wallet.tolk"</>
     output<dim> = </><green>"wallet.boc"</>
     depends<dim> = [</><green>"child"</><dim>]</>
     <dim># or as library with custom function name and output path</>
     depends<dim> = </><dim>[</>
       <dim>{{</> name<dim> = </><green>"child"</><dim>,</> kind<dim> = </><green>"library_ref"</><dim>,</> function<dim> = </><green>"getChildCode"</><dim>,</> path<dim> = </><green>"child_dep.tolk"</> <dim>}}</>
     <dim>]</>"#
    );

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
    let _ = write!(writer, "\n\n{header}Examples:{header:#}");

    const USAGE_SEP: &str = "\n     ";
    for (name, value) in build_examples.iter() {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{literal}{value}{literal:#}");
    }

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
    for (name, value) in disasm_examples.iter() {
        let _ = write!(writer, "{USAGE_SEP}{named}# {name}{named:#}");
        let _ = writeln!(writer, "{USAGE_SEP}{literal}{value}{literal:#}");
    }

    writer
}

fn main() {
    CompleteEnv::with_factory(Cli::command).complete();

    setup_panic!(
        Metadata::new("Acton", env!("CARGO_PKG_VERSION"))
            .authors("TON Core")
            .homepage("https://github.com/i582/acton")
    );
    dotenv().ok();
    setup_logging().expect("Failed to set up logging");
    let cli = Cli::parse();

    let result = match cli.command {
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
        } => {
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
                save_test_trace,
                mutate,
                mutate_overrides,
                mutate_contract,
                disable_rule,
                Some(fail_fast),
            );

            if mutate {
                mutation::test_mutate_cmd(&path, &config)
            } else {
                test_cmd(path, &config)
            }
        }
        Commands::Run { script, args } => run_cmd(&script, &args),
        Commands::Retrace {
            hash,
            net,
            api_key,
            verbose,
            logs_dir,
        } => retrace_cmd(hash, net.map(|n| n.to_string()), api_key, verbose, logs_dir),
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
            fork_net.map(|n| n.to_string()),
            api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
            fork_block_number,
            broadcast,
            net.map(|n| n.to_string()),
            explorer,
        ),
        Commands::Build {
            contract_id,
            clear_cache,
            graph,
            out_dir,
            info,
        } => build_cmd(contract_id, clear_cache, graph, out_dir, info),
        Commands::Compile {
            path,
            json,
            base64_only,
            boc,
            fift,
            source_map,
            clear_cache,
        } => {
            let result = compile_cmd(&path, json, base64_only, boc, fift, source_map, clear_cache);
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
                net.to_string(),
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
            net.to_string(),
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
            } => publish_cmd(
                contract_id,
                code,
                duration,
                wallet,
                api_key.or_else(|| env::var("TONCENTER_API_KEY").ok()),
                net.to_string(),
                amount,
                yes,
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
                    net.to_string(),
                    json,
                );
                if json {
                    if let Err(err) = result {
                        println!(
                            "{}",
                            serde_json::json!({
                                "success": false,
                                "error": err.to_string()
                            })
                        );
                    }
                    return;
                }
                result
            }
        },
        Commands::Up {
            version,
            canary,
            stable,
            yes,
        } => up_cmd(version, canary, stable, yes),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "acton", &mut std::io::stdout());
            Ok(())
        }
        Commands::Docgen { output } => docgen_cmd(output),
    };

    if let Err(err) = result {
        eprintln!("{} {}", "Error:".red(), err);
        process::exit(1)
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
            ))
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
    debug_port: u16,
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
) -> TestConfig {
    let acton_config = ActonConfig::load().ok();

    if let Some(acton_config) = acton_config
        && let Some(test_settings) = &acton_config.test
    {
        return test_settings.to_test_config(
            filter,
            report_formats,
            if debug { Some(true) } else { None },
            Some(debug_port),
            backtrace,
            if coverage { Some(true) } else { None },
            coverage_format,
            coverage_file,
            if !exclude.is_empty() {
                Some(exclude)
            } else {
                None
            },
            if !include.is_empty() {
                Some(include)
            } else {
                None
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
        );
    }

    TestConfig {
        debug,
        debug_port,
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
        fork_net,
        api_key,
        fork_block_number,
        save_test_trace,
        mutate,
        mutate_overrides,
        mutate_contract,
        disable_rules,
        fail_fast: fail_fast.unwrap_or(false),
    }
}
