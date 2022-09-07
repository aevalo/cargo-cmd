use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::convert::TryFrom;
use glob::glob;
use toml;

use crate::error::{Error, ErrorKind};

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

fn extend_manifest_paths(patterns: Vec<String>, excludes: Vec<PathBuf>) -> Result<Vec<String>, Error> {
    let mut manifest_paths = vec![];
    let path_bufs = extend_globs(patterns, excludes)?;
    for path_buf in path_bufs {
        if let Some(manifest_path) = path_buf.join("Cargo.toml").to_str() {
            manifest_paths.push(String::from(manifest_path));
        } else {
            return Err(Error {
                kind: ErrorKind::PathBufConversionError(format!("{:?}", path_buf)),
                message: String::from("Failed to convert path to string"),
            });
        }
    }
    Ok(manifest_paths)
}

fn extend_globs(patterns: Vec<String>, excludes: Vec<PathBuf>) -> Result<Vec<PathBuf>, Error> {
    let mut path_bufs = vec![];
    for pattern in patterns {
        match glob(pattern.as_str()) {
            Err(error) => return Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Invalid glob pattern \"{}\"", pattern),
            }),
            Ok(paths) => {
                for path in paths {
                    match path {
                        Err(error) => return Err(Error {
                            kind: ErrorKind::from(error),
                            message: String::from("Error reading path for globbing"),
                        }),
                        Ok(path) => {
                            if !excludes.contains(&path) {
                                path_bufs.push(path)
                            }
                        }
                    }
                }
            },
        }
    }
    Ok(path_bufs)
}

pub trait GetCommands {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error>;
}

#[derive(Deserialize, Debug)]
pub struct Package {
    metadata: Metadata,
}

impl GetCommands for Package {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error> {
        let mut commands = vec![];
        let names = vec![
            format!("pre{}", command),
            command.to_string(),
            format!("post{}", command),
        ];
        
        let cargo_commands = &self.metadata.commands;

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
    }
}

impl TryFrom<toml::Value> for Package {
    type Error = Error;

    fn try_from(value: toml::Value) -> Result<Self, Self::Error> {
        match value.try_into::<Package>() {
            Err(error) => return Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Failed to convert package"),
            }),
            Ok(package) => Ok(package)
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Workspace {
    pub members: Vec<Package>,
    pub metadata: Metadata,
}

impl GetCommands for Workspace {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error> {
        let mut commands = vec![];
        let names = vec![
            format!("pre{}", command),
            command.to_string(),
            format!("post{}", command),
        ];
        
        let cargo_commands = &self.metadata.commands;

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
        for member in self.members.iter() {
            let mut package_commands = member.get_commands(command)?;
            commands.append(&mut package_commands);
        }
        Ok(commands)
    }
}

impl TryFrom<toml::Value> for Workspace {
    type Error = Error;

    fn try_from(value: toml::Value) -> Result<Self, Self::Error> {
        if value.is_table() {
            let members = if let Some(members) = value.get("members") {
                let patterns = members.clone().try_into::<Vec<String>>().map_err(|error| Error {
                    kind: ErrorKind::from(error),
                    message: format!("Failed to convert workspace members"),
                })?;
                let excludes = value.get("exclude").map_or(Ok(vec![]), |exclude| {
                    exclude.clone().try_into::<Vec<PathBuf>>().map_err(|error| Error {
                        kind: ErrorKind::from(error),
                        message: format!("Failed to convert workspace excludes"),
                    })
                })?;
                let manifest_paths = extend_manifest_paths(patterns, excludes)?;
                let packages: Result<Vec<Package>, Error> = manifest_paths.iter().map(|path| {
                    let cargo_toml = CargoToml::from_path(path)?;
                    if let CargoToml::Package { path, package } = cargo_toml {
                        Ok(package)
                    } else {
                        Err(Error {
                            kind: ErrorKind::MalformedManifest(String::from("Only package members are currently supported")),
                            message: format!("Failed to convert workspace"),
                        })
                    }
                }).collect();
                packages
            } else {
                Err(Error {
                    kind: ErrorKind::MalformedManifest(String::from("Workspace does not contain members")),
                    message: format!("Failed to convert workspace"),
                })
            };

            let metadata = if let Some(value) = value.get("metadata") {
                Metadata::try_from(value.clone())?
            } else {
                Metadata {
                    commands: HashMap::new(),
                }
            };
            match members {
                Err(error) => Err(error),
                Ok(members) => Ok(Workspace {
                    members: members,
                    metadata: metadata,
                })
            }
        } else {
            Err(Error {
                kind: ErrorKind::MalformedManifest(String::from("Workspace is not a table")),
                message: format!("Failed to convert workspace"),
            })
        }
    }
}


#[derive(Deserialize, Debug)]
pub struct Metadata {
    commands: HashMap<String, String>,
}

impl TryFrom<toml::Value> for Metadata {
    type Error = Error;

    fn try_from(value: toml::Value) -> Result<Self, Self::Error> {
        match value.try_into::<Metadata>() {
            Err(error) => return Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Failed to convert metadata"),
            }),
            Ok(metadata) => Ok(metadata)
        }
    }
}

#[derive(Debug)]
pub enum CargoToml {
    Package {
        path: String,
        package: Package,
    },
    RootPackage {
        path: String,
        package: Package,
        workspace: Workspace,
    },
    VirtualManifest {
        path: String,
        workspace: Workspace,
    }
}

impl CargoToml {
    // Read Cargo.toml from path
    pub fn from_path(path: &str) -> Result<CargoToml, Error> {
        let content = read_file(path)?;
        let value = match content.parse::<toml::Value>() {
            Err(error) => return Err(Error {
                kind: ErrorKind::from(error),
                message: format!("Failed to parse \"{}\"", path),
            }),
            Ok(value) => value
        };
        let ret = if let Some(table) = value.as_table() {
            let package = if let Some(value) = table.get("package") {
                let pkg = Package::try_from(value.clone())?;
                Some(pkg)
            } else {
                None
            };
            let workspace = if let Some(value) = table.get("workspace") {
                let workspace = Workspace::try_from(value.clone())?;
                if package.is_some() {
                    Ok(CargoToml::RootPackage {
                        path: String::from(path),
                        package: Package {
                            metadata: package.map(|pkg| pkg.metadata).unwrap(),
                        },
                        workspace: workspace,
                    })
                } else {
                    Ok(CargoToml::VirtualManifest {
                        path: String::from(path),
                        workspace: workspace,
                    })
                }
            } else {
                if package.is_some() {
                    Ok(CargoToml::Package {
                        path: String::from(path),
                        package: package.unwrap(),
                    })
                } else {
                    Err(Error {
                        kind: ErrorKind::MalformedManifest(String::from(path)),
                        message: String::from("Manifest does not contain neither package or workspace"),
                    })            
                }
            };
            workspace
        } else {
            return Err(Error {
                kind: ErrorKind::MalformedManifest(String::from(path)),
                message: String::from("Manifest is not a table"),
            });       
        };
        ret
    }
}
