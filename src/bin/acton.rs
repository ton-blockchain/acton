use anyhow;
use clap::{Parser, Subcommand};
use emulator_rs::commands::compile::compile_cmd;
use emulator_rs::commands::disasm::disasm_cmd;
use emulator_rs::commands::init::init_cmd;
use emulator_rs::commands::new::new_cmd;
use emulator_rs::commands::script::script_cmd;
use emulator_rs::commands::test::test_cmd;
use owo_colors::OwoColorize;
use std::fs::OpenOptions;

#[derive(Parser)]
#[command(name = "acton")]
#[command(about = "TON blockchain development tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize a new project")]
    Init,
    #[command(about = "Create a new project in the specified directory")]
    New {
        #[arg(
            help = "Directory to create the project in (use '.' to create a project in the current directory)"
        )]
        path: String,
    },
    #[command(about = "Execute tests in file or directory")]
    Test {
        #[arg(help = "Test file or directory containing test files (default: current directory)")]
        path: Option<String>,
        #[arg(short, long, help = "Filter tests by regex pattern")]
        filter: Option<String>,
        #[arg(long, help = "Output in TeamCity format for IDE integration")]
        teamcity: bool,
        #[arg(long, help = "Enable debug mode")]
        debug: bool,
        #[arg(long, help = "Debug server port", default_value = "12345")]
        debug_port: u16,
        #[arg(long, help = "Enable backtraces")]
        backtrace: Option<String>,
        #[arg(long, help = "Enable coverage collection")]
        coverage: bool,
        #[arg(long, help = "Output coverage in specified format (lcov)")]
        format: Option<String>,
    },
    #[command(about = "Execute a Tolk script file")]
    Script {
        #[arg(help = "Script file to execute")]
        path: String,
        #[arg(long, help = "Enable debug mode")]
        debug: bool,
        #[arg(long, help = "Debug server port", default_value = "12345")]
        debug_port: u16,
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
    },
    #[command(about = "Disassemble TVM bitcode to human-readable TASM")]
    Disasm {
        #[arg(help = "Binary BoC file to disassemble (se -s flag to pass a string)")]
        boc_file: Option<String>,
        #[arg(short, long, help = "BoC string in hex or base64 format")]
        string: Option<String>,
        #[arg(short, long, help = "Output file (if not specified, output to stdout)")]
        output: Option<String>,
    },
}

fn main() {
    setup_logging().expect("Failed to set up logging");
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let result = init_cmd();
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::New { path } => {
            let result = new_cmd(&path);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::Test {
            path,
            filter,
            teamcity,
            debug,
            debug_port,
            backtrace,
            coverage,
            format,
        } => {
            let result = test_cmd(
                &path.unwrap_or_else(|| ".".to_string()),
                filter.as_deref(),
                teamcity,
                debug,
                debug_port,
                backtrace,
                coverage,
                format.as_deref(),
            );
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::Script {
            path,
            debug,
            debug_port,
        } => {
            let result = script_cmd(&path, debug, debug_port);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::Compile {
            path,
            json,
            base64_only,
            boc,
        } => {
            let result = compile_cmd(&path, json, base64_only, boc);
            if let Err(err) = result {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "success": false,
                            "error": err.to_string()
                        }))
                        .unwrap()
                    );
                } else {
                    eprintln!("{} {}", "Error:".red(), err);
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
            }
        }
    }
}

fn setup_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug.log")?;

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
