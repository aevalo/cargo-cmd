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

fn extend_globs(patterns: Vec<String>, excludes: Vec<String>) -> Result<Vec<PathBuf>, Error> {
    let exclude_path_bufs = excludes.iter().map(|exclude| PathBuf::from(exclude)).collect::<Vec<PathBuf>>();
    let mut paths = vec![];
    for pattern in patterns {
        let path_bufs = extend_glob(pattern.as_str())?;
        path_bufs.iter().for_each(|path_buf| {
            if !exclude_path_bufs.contains(path_buf) {
                paths.push(path_buf.to_owned())
            }
        });
    };
    Ok(paths)
}

fn extend_glob(pattern: &str) -> Result<Vec<PathBuf>, Error> {
    match glob(pattern) {
        Err(error) => Err(Error {
            kind: ErrorKind::from(error),
            message: format!("Invalid glob pattern \"{}\"", pattern),
        }),
        Ok(paths) => {
            let mut path_bufs = vec![];
            for path in paths {
                match path {
                    Err(error) => return Err(Error {
                        kind: ErrorKind::from(error),
                        message: String::from("Error reading path for globbing"),
                    }),
                    Ok(path) => path_bufs.push(path)
                }
            };
            Ok(path_bufs)
        },
    }
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
                if members.is_array() {
                    let excludes = value.get("exclude").map_or(Ok(vec![]), |exclude| {
                        exclude.clone().try_into::<Vec<String>>().map_err(|error| Error {
                            kind: ErrorKind::from(error),
                            message: format!("Failed to convert workspace excludes"),
                        })
                    })?;
                    let patterns = members.clone().try_into::<Vec<String>>().map_err(|error| Error {
                        kind: ErrorKind::from(error),
                        message: format!("Failed to convert workspace members"),
                    })?;
                    let path_bufs = extend_globs(patterns, excludes)?;
                    let packages: Result<Vec<Package>, Error> = path_bufs.iter().map(|path| {
                        if let Some(cargo_toml_path) = path.join("Cargo.toml").to_str() {
                            let cargo_toml = CargoToml::from_path(cargo_toml_path)?;
                            if let CargoToml::Package { path, package } = cargo_toml {
                                Ok(package)
                            } else {
                                Err(Error {
                                    kind: ErrorKind::MalformedManifest(String::from("Only package members are currently supported")),
                                    message: format!("Failed to convert workspace"),
                                })
                            }
                        } else {
                            Err(Error {
                                kind: ErrorKind::PathBufConversionError(format!("{:?}", path)),
                                message: String::from("Failed to convert path to string"),
                            })
                        }
                    }).collect();
                    packages
                } else {
                    Err(Error {
                        kind: ErrorKind::MalformedManifest(String::from("Workspace members is not an array")),
                        message: format!("Failed to convert workspace"),
                    })
                }
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
