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
use std::convert::TryFrom;
//use cargo::GetCommands;

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

mod cargo;

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
    let mut commands = vec![];
    /*let content = cargo::read_file("Cargo.toml").map_err(|err| format!("{}", err))?;
    let toml = content.parse::<toml::Value>().unwrap();
    if let toml::Value::Table(cargo_toml) = toml {
        if cargo_toml.contains_key("package") {
            let package = cargo_toml.get("package").unwrap();
            println!("{:?}", package);
            let metadata = package.get("metadata").unwrap();
            println!("{:?}", metadata);
            let commands = metadata.get("commands").unwrap();
            println!("{:?}", commands);
            let pass: Option<&str> = commands.get("pass").and_then(|value| value.as_str());
            println!("{:?}", pass);
            let nonexistent: Option<&str> = commands.get("nonexistent").and_then(|value| value.as_str());
            println!("{:?}", nonexistent);
        }
    }*/
    let cargo_toml = cargo::CargoToml::from_path("Cargo.toml").map_err(|err| format!("{}", err))?;
    println!("{:?}", cargo_toml);
    /*if cargo_toml.is_package() {
        let package = cargo::Package::try_from(cargo_toml).unwrap();
        let mut package_commands = package.get_commands(command).map_err(|err| format!("{}", err))?;
        commands.append(&mut package_commands);
    } else if cargo_toml.is_root_package() {
        let root_package = cargo::RootPackage::try_from(cargo_toml).unwrap();
        let mut root_package_commands = root_package.get_commands(command).map_err(|err| format!("{}", err))?;
        commands.append(&mut root_package_commands);
    }
    else if cargo_toml.is_virtual_manifest() {
        let virtual_manifest = cargo::VirtualManifest::try_from(cargo_toml).unwrap();
        let mut workspace_commands = virtual_manifest.get_commands(command).map_err(|err| format!("{}", err))?;
        commands.append(&mut workspace_commands);
    }*/
    Ok(commands)
}
