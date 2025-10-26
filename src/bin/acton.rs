use clap::{Parser, Subcommand};
use emulator_rs::commands::compile::compile_cmd;
use emulator_rs::commands::init::init_cmd;
use emulator_rs::commands::new::new_cmd;
use emulator_rs::commands::script::script_cmd;
use emulator_rs::commands::test::test_cmd;
use owo_colors::OwoColorize;

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
        #[arg(help = "Test file or directory containing test files")]
        path: String,
        #[arg(short, long, help = "Filter tests by regex pattern")]
        filter: Option<String>,
        #[arg(long, help = "Output in TeamCity format for IDE integration")]
        teamcity: bool,
    },
    #[command(about = "Execute a Tolk script file")]
    Script {
        #[arg(help = "Script file to execute")]
        path: String,
        #[arg(long, help = "Enable debug mode")]
        debug: bool,
    },
    #[command(about = "Compile a Tolk file")]
    Compile {
        #[arg(help = "Tolk file to compile")]
        path: String,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
        #[arg(long, help = "Output only base64 code")]
        base64_only: bool,
    },
}

fn main() {
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
        } => {
            let result = test_cmd(&path, filter.as_deref(), teamcity);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::Script { path, debug } => {
            let result = script_cmd(&path, debug);
            if let Err(err) = result {
                eprintln!("{} {}", "Error:".red(), err);
            }
        }
        Commands::Compile {
            path,
            json,
            base64_only,
        } => {
            let result = compile_cmd(&path, json, base64_only);
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
    }
}
