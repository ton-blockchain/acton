use acton::commands;
use acton::commands::build::build_cmd;
use acton::commands::compile::compile_cmd;
use acton::commands::disasm::disasm_cmd;
use acton::commands::init::init_cmd;
use acton::commands::new::new_cmd;
use acton::commands::script::script_cmd;
use acton::commands::test::{ReportFormat, TestConfig, mutation, test_cmd};
use acton::commands::test_gen::test_gen_cmd;
use acton::commands::verify::verify_cmd;
use acton::config::ActonConfig;
use clap::builder::styling::Style;
use clap::builder::{StyledStr, Styles};
use clap::{ColorChoice, CommandFactory};
use clap::{Parser, Subcommand, arg};
use commands::common::error_fmt;
use dotenvy::dotenv;
use human_panic::{Metadata, setup_panic};
use owo_colors::OwoColorize;
use std::fs::OpenOptions;
use std::{fs, process};
use tasm::printer::FormatOptions;
use tolkc::source_map::SourceMap;

#[derive(Parser)]
#[command(name = "acton")]
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
        long_about = "Initialize a new project in the current directory. Suitable if you want to use Acton in an existing project."
    )]
    Init,
    #[command(
        about = "Create a new project in the specified directory",
        long_about = "Create a new project in the specified directory. Suitable if you want to create a new project with Acton."
    )]
    New {
        #[arg(
            help = "Directory to create the project in (use '.' to create a project in the current directory)"
        )]
        path: String,
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
        #[arg(
            long,
            help = "Enable backtraces (currently only \"full\" mode is available)",
            help_heading = "Debugging"
        )]
        backtrace: Option<String>,

        // Coverage
        #[arg(long, help = "Generate a coverage profile", help_heading = "Coverage")]
        coverage: bool,
        #[arg(
            long,
            help = "Output coverage profile in specified format (lcov, text)",
            help_heading = "Coverage"
        )]
        coverage_format: Option<String>,
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
        reporter: Vec<String>,
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
            help = "Fork from network (testnet or mainnet) for remote account resolution",
            help_heading = "Remote"
        )]
        fork_net: Option<String>,
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
            help = "Contract ID to mutate during mutation testing",
            help_heading = "Mutation Testing",
            value_name = "ID"
        )]
        mutate_contract: Option<String>,
    },
    #[command(about = "Generate test wrapper and test file for a contract")]
    TestGen {
        #[arg(help = "Contract ID from Acton.toml")]
        contract_id: String,
        #[arg(long, help = "Output path for wrapper file")]
        wrapper_output: Option<String>,
        #[arg(long, help = "Output path for test file")]
        test_output: Option<String>,
    },
    #[command(about = "Execute a Tolk script file")]
    Script {
        #[arg(help = "Script file to execute")]
        path: String,

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
            help = "Fork from network (testnet or mainnet) for remote account resolution",
            help_heading = "Remote"
        )]
        fork_net: Option<String>,
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
            help = "Network to use for broadcasting (mainnet, testnet)",
            default_value = "testnet",
            help_heading = "Broadcasting"
        )]
        net: String,
    },
    #[command(
        about = "Build the specified contract or all contracts",
        after_help = example_build_usage()
    )]
    Build {
        #[arg(help = "Contract name to build (builds all if not specified)")]
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
        #[arg(long, help = "TonCenter API key (optional)")]
        api_key: Option<String>,
        #[arg(
            long,
            help = "Network to use for fetching libraries (testnet or mainnet)",
            default_value = "mainnet"
        )]
        net: String,
        #[arg(
            long,
            help = "Follow library references and disassemble the actual library code instead of showing library hash"
        )]
        follow_libraries: bool,
    },
    #[command(about = "Verify contract source code on verifier.ton.org")]
    Verify {
        #[arg(help = "Contract ID from Acton.toml (optional, will prompt if not provided)")]
        contract: Option<String>,
        #[arg(
            long,
            help = "Deployed contract address (optional, will prompt if not provided)"
        )]
        address: Option<String>,
        #[arg(
            long,
            help = "Network to use (mainnet or testnet)",
            default_value = "testnet"
        )]
        net: String,
        #[arg(
            long,
            help = "Wallet from Acton.toml to use for verification (optional, will use default if only one wallet configured)"
        )]
        wallet: Option<String>,
        #[arg(
            long,
            help = "Tolk compiler version to use on verifier side (optional)"
        )]
        compiler_version: Option<String>,
        #[arg(long, help = "Run verification without sending the final transaction")]
        dry_run: bool,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
    },
    #[command(
        about = "Generate shell completions for selected shell",
        after_help = "For installation instructions, see https://acton.dev/acton/shell-completions/"
    )]
    Completions {
        #[clap(value_enum)]
        shell: clap_complete::Shell,
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

fn example_build_usage() -> StyledStr {
    use std::fmt::Write as _;

    let mut writer = StyledStr::new();
    let styled = Styles::styled();

    // for some reason `cstr` cannot output `{` correctly :/
    let config_example = color_print::cformat!(
        r#"<dim>[</>contracts.wallet<dim>]</>
     name<dim> = </><green>"Wallet Contract"</>
     root<dim> = </><green>"contracts/wallet.tolk"</>
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
        Commands::New { path } => new_cmd(&path),
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
        } => {
            if mutate {
                mutation::test_mutate_cmd(&path, mutate_contract)
            } else {
                let mut report_formats = Vec::new();

                for format_str in reporter {
                    match format_str.to_lowercase().as_str() {
                        "console" => report_formats.push(ReportFormat::Console),
                        "teamcity" => report_formats.push(ReportFormat::TeamCity),
                        "junit" => report_formats.push(ReportFormat::JUnit),
                        "dot" => report_formats.push(ReportFormat::Dot),
                        _ => {
                            eprintln!(
                                "Warning: Unknown report format '{format_str}'. Supported formats: console, teamcity, junit, dot"
                            );
                        }
                    }
                }

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
                    report_formats,
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
                );
                test_cmd(path, &config)
            }
        }
        Commands::TestGen {
            contract_id,
            wrapper_output,
            test_output,
        } => test_gen_cmd(&contract_id, wrapper_output, test_output),
        Commands::Script {
            path,
            debug,
            debug_port,
            clear_cache,
            fork_net,
            api_key,
            broadcast,
            net,
        } => script_cmd(
            &path,
            debug,
            debug_port,
            clear_cache,
            fork_net,
            api_key,
            broadcast,
            net,
        ),
        Commands::Build {
            contract_id,
            clear_cache,
            graph,
            out_dir,
        } => build_cmd(contract_id, clear_cache, graph, out_dir),
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
                api_key,
                net,
                follow_libraries,
            ),
            Err(err) => Err(err),
        },
        Commands::Verify {
            contract,
            address,
            net,
            wallet,
            compiler_version,
            dry_run,
            api_key,
        } => verify_cmd(
            contract,
            address,
            net,
            wallet,
            compiler_version,
            dry_run,
            api_key,
        ),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "acton", &mut std::io::stdout());
            Ok(())
        }
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
    backtrace: Option<String>,
    coverage: bool,
    coverage_format: Option<String>,
    coverage_file: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    clear_cache: bool,
    report_formats: Vec<ReportFormat>,
    junit_path: Option<String>,
    junit_merge: bool,
    snapshot: Option<String>,
    baseline_snapshot: Option<String>,
    fork_net: Option<String>,
    api_key: Option<String>,
    save_test_trace: Option<String>,
    mutate: bool,
    mutate_overrides: Option<String>,
    mutate_contract: Option<String>,
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
            save_test_trace,
            mutate,
            mutate_overrides,
            mutate_contract,
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
        save_test_trace,
        mutate,
        mutate_overrides,
        mutate_contract,
    }
}
