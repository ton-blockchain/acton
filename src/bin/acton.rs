use clap::ColorChoice;
use clap::builder::styling::Style;
use clap::builder::{StyledStr, Styles};
use clap::{Parser, Subcommand, arg};
use emulator_rs::commands::build::build_cmd;
use emulator_rs::commands::compile::compile_cmd;
use emulator_rs::commands::disasm::disasm_cmd;
use emulator_rs::commands::init::init_cmd;
use emulator_rs::commands::new::new_cmd;
use emulator_rs::commands::script::script_cmd;
use emulator_rs::commands::test::{ReportFormat, TestConfig, test_cmd};
use emulator_rs::config::ActonConfig;
use owo_colors::OwoColorize;
use std::fs::OpenOptions;
use std::{fs, process};

#[derive(Parser)]
#[command(name = "acton")]
#[command(about = "TON blockchain development tool")]
#[command(color = ColorChoice::Auto)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

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
        #[arg(short, long, help = "Filter tests by regex pattern")]
        filter: Option<String>,
        #[arg(long, help = "Report formats to use", value_delimiter = ',')]
        reporter: Vec<String>,
        #[arg(long, help = "Enable debug mode")]
        debug: bool,
        #[arg(long, help = "Debug server port", default_value = "12345")]
        debug_port: u16,
        #[arg(
            long,
            help = "Enable backtraces (currently only \"full\" mode is available)"
        )]
        backtrace: Option<String>,
        #[arg(long, help = "Generate a coverage profile")]
        coverage: bool,
        #[arg(long, help = "Output coverage profile in specified format (lcov)")]
        format: Option<String>,
        #[arg(
            long,
            help = "Exclude test files and directories matching glob patterns"
        )]
        exclude: Vec<String>,
        #[arg(
            long,
            help = "Include only test files and directories matching glob patterns"
        )]
        include: Vec<String>,
        #[arg(long, help = "Clear compilation cache before running")]
        clear_cache: bool,
        #[arg(
            long,
            default_value = "test-results",
            help = "JUnit XML output directory"
        )]
        junit_path: Option<String>,
        #[arg(long, help = "Merge all test suites into a single JUnit XML file")]
        junit_merge: bool,
    },
    #[command(about = "Execute a Tolk script file")]
    Script {
        #[arg(help = "Script file to execute")]
        path: String,
        #[arg(long, help = "Enable debug mode")]
        debug: bool,
        #[arg(long, help = "Debug server port", default_value = "12345")]
        debug_port: u16,
        #[arg(long, help = "Clear compilation cache before running")]
        clear_cache: bool,
    },
    #[command(
        about = "Build all contracts",
        after_help = example_build_usage()
    )]
    Build {
        #[arg(help = "Contract name to build (builds all if not specified)")]
        contract: Option<String>,
        #[arg(long, help = "Clear compilation cache before building")]
        clear_cache: bool,
        #[arg(
            long,
            help = "Generate dependency graph as SVG file (requires graphviz)"
        )]
        graph: Option<String>,
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
        #[arg(long, help = "Clear compilation cache before running")]
        clear_cache: bool,
    },
    #[command(about = "Disassemble TVM bitcode to human-readable TASM")]
    Disasm {
        #[arg(help = "Binary BoC file to disassemble (use -s flag to pass a string)")]
        boc_file: Option<String>,
        #[arg(short, long, help = "BoC string in hex or base64 format")]
        string: Option<String>,
        #[arg(short, long, help = "Output file (if not specified, output to stdout)")]
        output: Option<String>,
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
            "acton test . --coverage --format lcov",
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

fn main() {
    setup_logging().expect("Failed to set up logging");
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let result = init_cmd();
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1)
            }
        }
        Commands::New { path } => {
            let result = new_cmd(&path);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1)
            }
        }
        Commands::Test {
            path,
            filter,
            reporter,
            debug,
            debug_port,
            backtrace,
            coverage,
            format,
            exclude,
            include,
            clear_cache,
            junit_path,
            junit_merge,
        } => {
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
                format,
                exclude,
                include,
                clear_cache,
                report_formats,
                junit_path,
                junit_merge,
            );
            let result = test_cmd(path, &config);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1)
            }
        }
        Commands::Script {
            path,
            debug,
            debug_port,
            clear_cache,
        } => {
            let result = script_cmd(&path, debug, debug_port, clear_cache);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1)
            }
        }
        Commands::Build {
            contract,
            clear_cache,
            graph,
        } => {
            let result = build_cmd(contract, clear_cache, graph);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1);
            }
        }
        Commands::Compile {
            path,
            json,
            base64_only,
            boc,
            fift,
            clear_cache,
        } => {
            let result = compile_cmd(&path, json, base64_only, boc, fift, clear_cache);
            if let Err(err) = result {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "success": false,
                            "error": err.to_string()
                        }))
                        .expect("JSON serialization should not fail")
                    );
                } else {
                    eprintln!("{} {}", "Error:".red(), err);
                    process::exit(1)
                }
            }
        }
        Commands::Disasm {
            boc_file,
            string,
            output,
        } => {
            let result = disasm_cmd(boc_file, string, output);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
                process::exit(1)
            }
        }
    }
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
    format: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    clear_cache: bool,
    report_formats: Vec<ReportFormat>,
    junit_path: Option<String>,
    junit_merge: bool,
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
            format,
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
        );
    }

    TestConfig {
        debug,
        debug_port,
        backtrace,
        coverage,
        filter,
        coverage_format: format,
        exclude_patterns: exclude,
        include_patterns: include,
        clear_cache,
        report_formats,
        junit_path,
        junit_merge,
    }
}
