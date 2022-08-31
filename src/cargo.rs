use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::convert::{From, TryFrom};
use glob::glob;
use toml;

#[derive(Deserialize, Debug)]
pub struct CargoToml {
    pub workspace: Option<WorkspaceToml>,
    pub package: Option<Package>,
}

impl CargoToml {
    // Does the Cargo.toml contain just a simple package?
    pub fn is_package(&self) -> bool {
        self.workspace.is_none() && self.package.is_some()
    }

    // Does the Cargo.toml contain just a root package with workspace?
    pub fn is_root_package(&self) -> bool {
        self.workspace.is_some() && self.package.is_some()
    }

    // Does the Cargo.toml contain virtual manifest (just a workspace)?
    pub fn is_virtual_manifest(&self) -> bool {
        self.workspace.is_some() && self.package.is_none()
    }
}

pub trait GetCommands {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error>;
}

#[derive(Deserialize, Debug)]
pub struct WorkspaceToml {
    pub members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub metadata: Option<Metadata>,
}

#[derive(Deserialize, Debug)]
pub struct Package {
    metadata: Option<Metadata>,
}

impl GetCommands for Package {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error> {
        if let Some(metadata) = &self.metadata {
            let mut commands = vec![];
            let names = vec![
                format!("pre{}", command),
                command.to_string(),
                format!("post{}", command),
            ];
            
            let cargo_commands = &metadata.commands;
    
            for name in names {
                let command_to_run = cargo_commands.get(&name);
        
                if name == command && command_to_run.is_none() {
                    return Err(Error {
                        kind: ErrorKind::MissingCommand(String::from(command)),
                        message: String::new(),
                    });
                }
        
                if command_to_run.is_some() {
                    commands.push((name, command_to_run.unwrap().to_string()));
                }
            }
    
            Ok(commands)
        } else {
            Ok(vec![])
        }
    }
}

impl TryFrom<CargoToml> for Package {
    type Error = Error;

    fn try_from(cargo_toml: CargoToml) -> Result<Self, Self::Error> {
        if cargo_toml.is_package() {
            Ok(cargo_toml.package.unwrap())
        } else {
            Err(Error {
                kind: ErrorKind::NotPackage,
                message: String::new(),
            })            
        }
    }
}

#[derive(Debug)]
pub struct RootPackage {
    pub workspace: Workspace,
    pub package: Package,
}

impl GetCommands for RootPackage {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error> {
        let mut package_commands = self.package.get_commands(command)?;
        let mut workspace_commands = self.workspace.get_commands(command)?;
        package_commands.append(&mut workspace_commands);
        Ok(package_commands)
    }
}

impl TryFrom<CargoToml> for RootPackage {
    type Error = Error;

    fn try_from(cargo_toml: CargoToml) -> Result<Self, Self::Error> {
        if cargo_toml.is_root_package() {
            Ok(RootPackage {
                workspace: Workspace::from(cargo_toml.workspace.unwrap()),
                package: cargo_toml.package.unwrap(),
            })
        } else {
            Err(Error {
                kind: ErrorKind::NotRootPackage,
                message: String::new(),
            })            
        }
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub members: Vec<Package>,
    pub metadata: Option<Metadata>,
}

impl GetCommands for Workspace {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error> {
        if let Some(metadata) = &self.metadata {
            let mut commands = vec![];
            let names = vec![
                format!("pre{}", command),
                command.to_string(),
                format!("post{}", command),
            ];
            
            let cargo_commands = &metadata.commands;
    
            for name in names {
                let command_to_run = &cargo_commands.get(&name);
        
                if name == command && command_to_run.is_none() {
                    return Err(Error {
                        kind: ErrorKind::MissingCommand(String::from(command)),
                        message: String::new(),
                    });
                }
        
                if command_to_run.is_some() {
                    commands.push((name, command_to_run.unwrap().to_string()));
                }
            }
            for package in self.members.iter() {
                let mut package_commands = package.get_commands(command)?;
                commands.append(&mut package_commands);
            }
            Ok(commands)
        } else {
            Ok(vec![])
        }
    }
}

impl TryFrom<CargoToml> for Workspace {
    type Error = Error;

    fn try_from(cargo_toml: CargoToml) -> Result<Self, Self::Error> {
        if cargo_toml.is_virtual_manifest() {
            Ok(Workspace::from(cargo_toml.workspace.unwrap()))
        } else {
            Err(Error {
                kind: ErrorKind::NotVirtualManifest,
                message: String::new(),
            })            
        }
    }
}

impl From<WorkspaceToml> for Workspace {
    fn from(workspace_toml: WorkspaceToml) -> Self {
        let excludes = workspace_toml.exclude.unwrap_or(vec![]);
        let members = if let Some(members) = workspace_toml.members {
            extend_globs(&members).iter().filter_map(|member| {
                match member {
                    Err(_error) => None,
                    Ok(path) => {
                        if path.to_str().map_or(false, |path_str| excludes.contains(&String::from(path_str))) {
                            None
                        } else {
                            Some(path.join("Cargo.toml"))
                        }
                    },
                }
            }).collect()
        } else {
            vec![]
        };
        let packages: Vec<Package> = members.iter().filter_map(|member| {
            member.to_str()
                .and_then(|path| read_file(path).ok())
                .and_then(|content| toml::from_str::<CargoToml>(&content).ok())
                .and_then(|cargo_toml|
                    if cargo_toml.is_package() {
                        cargo_toml.package
                    } else {
                        None
                    }
                )
        }).collect();
        Workspace {
            members: packages,
            metadata: workspace_toml.metadata,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Metadata {
    commands: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum ErrorKind {
    IoError(String),
    ParseError(String),
    GlobError(String),
    MissingCommand(String),
    NotPackage,
    NotRootPackage,
    NotVirtualManifest,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::IoError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::ParseError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::GlobError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::MissingCommand(command) => write!(f, "Command \"{}\" not found in Cargo.toml", command)?,
            ErrorKind::NotPackage => write!(f, "Cargo.toml does not contain a package")?,
            ErrorKind::NotRootPackage => write!(f, "Cargo.toml does not contain a root package")?,
            ErrorKind::NotVirtualManifest => write!(f, "Cargo.toml does not contain a virtual manifest")?,
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for ErrorKind {
    fn from(error: io::Error) -> Self {
        ErrorKind::IoError(format!("{}", error))
    }
}

impl From<toml::de::Error> for ErrorKind {
    fn from(error: toml::de::Error) -> Self {
        ErrorKind::ParseError(format!("{}", error))
    }
}

impl From<glob::PatternError> for ErrorKind {
    fn from(error: glob::PatternError) -> Self {
        ErrorKind::GlobError(format!("{}", error))
    }
}

impl From<glob::GlobError> for ErrorKind {
    fn from(error: glob::GlobError) -> Self {
        ErrorKind::GlobError(format!("{}", error))
    }
}

fn read_file(path: &str) -> Result<String, Error> {
    let mut file = match File::open(path) {
        Err(error) => {
            return Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Failed to open file \"{}\"", path),
            });
        },
        Ok(file) => file
    };
    let mut content = String::new();
    if let Err(error) = file.read_to_string(&mut content) {
        return Err(Error {
            kind: ErrorKind::from(error),
            message: format!("Failed to read file \"{}\"", path),
        });
    }
    Ok(content)
}

fn extend_globs(patterns: &Vec<String>) -> Vec<Result<PathBuf, Error>> {
    patterns.iter().map(|pattern| match extend_glob(pattern) {
        Err(error) => vec!(Err(error)),
        Ok(paths) => paths,
    }).flatten().collect()
}

fn extend_glob(pattern: &str) -> Result<Vec<Result<PathBuf, Error>>, Error> {
    match glob(pattern) {
        Err(error) => Err(Error {
            kind: ErrorKind::from(error),
            message: format!("Invalid glob pattern \"{}\"", pattern),
        }),
        Ok(paths) => {
            let mapped_paths = paths.map(|path| {
                match path {
                    Err(error) => Err(Error {
                        kind: ErrorKind::from(error),
                        message: String::from("Error reading path for globbing"),
                    }),
                    Ok(entry) => Ok(entry),
                }
            }).collect();
            Ok(mapped_paths)
        },
    }
}

pub fn from_path(path: &str) -> Result<CargoToml, Error> {
    match read_file(path) {
        Err(error) => Err(Error::from(error)),
        Ok(content) => match toml::from_str::<CargoToml>(&content) {
            Err(error) => Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Failed to parse file \"{}\"", path),
            }),
            Ok(cargo_toml) => Ok(cargo_toml),
        }
    }
}
