#[macro_use]
extern crate serde;
extern crate clap;
extern crate structopt;
extern crate subprocess;
extern crate toml;
extern crate glob;

use std::process;
use std::vec::Vec;
use structopt::StructOpt;
use subprocess::{Exec, ExitStatus};

mod cargo;
mod error;

use cargo::GetCommands;
use error::{Error, ErrorKind};

#[derive(StructOpt, Debug)]
#[structopt(name = "cargo-cmd", bin_name = "cargo")]
enum Cli {
    #[structopt(name = "cmd")]
    Cmd {
        #[structopt(name = "command", index = 1)]
        command: String,
        #[structopt(multiple = true)]
        rest: Vec<String>,
    },
}

fn main() {
    let cli = Cli::from_args();
    let (command, rest) = match cli {
        Cli::Cmd { command, rest } => (command, rest),
    };
    let commands = unwrap_or_exit(get_commands(&command));
    let is_multiple_commands = commands.len() > 1;

    for (index, command) in commands.iter().enumerate() {
        if is_multiple_commands {
            println!("\n[{}]", &command.0);
        }
        let command = &command.1;
        let exit = execute_command(command, &rest);

        if exit.success() {
            if index == commands.len() {
                process::exit(0);
            }
        } else {
            match exit {
                ExitStatus::Exited(exit_code) => process::exit(exit_code as i32),
                _ => process::exit(1),
            }
        }
    }
}

fn execute_command(command: &str, rest: &Vec<String>) -> ExitStatus {
    // This is naughty but Exec::shell doesn't let us do it with .args because
    // it ends up as an argument to sh/cmd.exe instead of our user command
    // or escaping things weirdly.
    let command = format!("{} {}", command, rest.join(" "));
    println!("> {}", command);
    let sh = Exec::shell(command);
    sh.join().unwrap_or(ExitStatus::Exited(0))
}

fn unwrap_or_exit<T>(result: Result<T, String>) -> T {
    match result {
        Err(error_msg) => {
            clap::Error::with_description(&error_msg[..], clap::ErrorKind::InvalidValue).exit();
        }
        Ok(thing) => thing,
    }
}

fn get_commands(command: &str) -> Result<Vec<(String, String)>, String> {
    let cargo_toml = cargo::CargoToml::from_path("Cargo.toml").map_err(|err| format!("{}", err))?;
    let commands = match cargo_toml {
        cargo::CargoToml::Package { path, package } => package.get_commands(command).map_err(|err| format!("{}", err))?,
        cargo::CargoToml::RootPackage { path, package, workspace } => {
            let mut commands = vec![];
            let package_commands = match package.get_commands(command) {
                Err(error) => {
                    if let error::ErrorKind::MissingCommand(reason) = error.kind {
                        vec![]
                    } else {
                        return Err(format!("{}", error));
                    }
                },
                Ok(commands) => commands
            };
            for command in package_commands {
                commands.push(command);
            }
            let workspace_commands = match workspace.get_commands(command) {
                Err(error) => {
                    if let error::ErrorKind::MissingCommand(reason) = error.kind {
                        vec![]
                    } else {
                        return Err(format!("{}", error));
                    }
                },
                Ok(commands) => commands
            };
            for command in workspace_commands {
                commands.push(command);
            }
            if commands.is_empty() {
                let error = Error {
                    kind: ErrorKind::MissingCommand(String::from(command)),
                    message: String::new(),
                };
                return Err(format!("{}", error));
            }
            commands
        },
        cargo::CargoToml::VirtualManifest { path, workspace } => workspace.get_commands(command).map_err(|err| format!("{}", err))?,
    };
    Ok(commands)
}
