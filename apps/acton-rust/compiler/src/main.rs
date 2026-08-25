use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::ExitCode;

use acton_rust_compiler::compile_source;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("acton-rustc: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let input = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            "usage: acton-rustc <contract.rs> [--output <contract.tolk>]",
        )
    })?;
    let mut output = None;

    while let Some(argument) = arguments.next() {
        if argument == "--output" || argument == "-o" {
            let path = arguments.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidInput, "--output requires a path")
            })?;
            output = Some(path);
        } else {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unknown argument `{}`", argument.to_string_lossy()),
            )
            .into());
        }
    }

    let source = fs::read_to_string(&input)?;
    let generated = compile_source(&source)?;

    if let Some(output) = output {
        fs::write(output, generated)?;
    } else {
        print!("{generated}");
    }

    Ok(())
}
