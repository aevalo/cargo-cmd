use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::convert::TryFrom;
use glob::glob;
use toml;

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

/*
impl TryFrom<Toml> for Package {
    type Error = Error;

    fn try_from(cargo_toml: Toml) -> Result<Self, Self::Error> {
        if cargo_toml.package.is_some() && cargo_toml.workspace.is_none() {
            std::debug_assert!(cargo_toml.package.is_some());
            let mut package = cargo_toml.package.unwrap();
            //package.path = cargo_toml.path;
            Ok(package)
        } else {
            Err(Error {
                kind: ErrorKind::NotPackage,
                message: String::new(),
            })            
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Workspace {
    pub members: Vec<Package>,
    pub metadata: Metadata,
}

impl TryFrom<Option<WorkspaceToml>> for Workspace {
    type Error = Error;

    fn try_from(workspace_toml: Option<WorkspaceToml>) -> Result<Self, Self::Error> {
        if let Some(workspace) = workspace_toml {
            let members = workspace.members().iter().map(|member| member.join("Cargo.toml")).collect::<Vec<PathBuf>>();
            let packages: Vec<Result<Package, Error>> = members.iter().map(|member| {
                member.to_str().map_or(Err(Error {
                    kind: ErrorKind::PathBufConversionError(format!("{:?}", member)),
                    message: String::from("Failed to convert path to string"),
                }), |path_str| {
                    let cargo_toml = CargoToml::from_path(path_str)?;
                    Package::try_from(cargo_toml)
                })
            }).collect();
            Ok(Workspace {
                members: packages,
                metadata: workspace.map_or(None, |ws| ws.metadata),
            })
        } else {
            Err(Error {
                kind: ErrorKind::MissingWorkspace(""),
                message: String::new(),
            })            
        }
    }
}
*/

#[derive(Deserialize, Debug)]
pub struct Package {
    metadata: Metadata,
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
                            if let CargoToml::Package { path, metadata } = cargo_toml {
                                Ok(Package {
                                    metadata: metadata,
                                })
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
        metadata: Metadata,
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
                        metadata: package.map(|pkg| pkg.metadata).unwrap(),
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
        /*let toml = Toml::from_path(path);
        if toml.package.is_some() {
            let package = toml.package.unwrap();
            if toml.workspace.is_some() {
                let workspace = Workspace::try_from(toml.workspace)?;
                Ok(CargoToml::RootPackage {
                    path: String::from(path),
                    package: package,
                    workspace: workspace,
                })
            } else {
                Ok(CargoToml::Package {
                    path: String::from(path),
                    metadata: package.metadata,
                })
            }
        } else if toml.package.is_none() && toml.workspace.is_some() {
            let workspace = Workspace::try_from(toml.workspace)?;
            Ok(CargoToml::Package {
                path: String::from(path),
                workspace: workspace,
            })
    } else {*/
            //Err(Error {
            //    kind: ErrorKind::MalformedManifest(String::from(path)),
            //    message: String::from("Manifest does not contain neither package or workspace"),
            //})            
    //    }
    }
}

/*#[derive(Deserialize, Debug)]
struct Toml {
    #[serde(skip)]
    pub path: String,
    pub workspace: Option<WorkspaceToml>,
    pub package: Option<Package>,
}

impl Toml {
    // Read Cargo.toml from path
    pub fn from_path(path: &str) -> Result<Toml, Error> {
        match read_file(path) {
            Err(error) => Err(Error::from(error)),
            Ok(content) => match toml::from_str::<Toml>(&content) {
                Err(error) => Err(Error {
                    kind: ErrorKind::from(error),
                    message: format!("Failed to parse file \"{}\"", path),
                }),
                Ok(mut cargo_toml) => {
                    cargo_toml.path = String::from(path);
                    Ok(cargo_toml)
                },
            }
        }
    }
}

#[derive(Deserialize, Debug)]
struct WorkspaceToml {
    pub members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub metadata: Option<Metadata>,
}

impl WorkspaceToml {
    fn excludes(&self) -> Vec<String> {
        match &self.exclude {
            None => vec![],
            Some(excludes) => excludes.to_vec(),
        }
    }

    pub fn members(&self) -> Vec<PathBuf> {
        match &self.members {
            None => vec![],
            Some(members) => {
                let excludes = self.excludes();
                extend_globs(&members).iter().filter_map(|member| {
                    match member {
                        Err(_) => None,
                        Ok(path) => {
                            path.to_str().map_or(None, |path_str| {
                                if excludes.contains(&String::from(path_str)) {
                                    Some(path.clone())
                                } else {
                                    None
                                }
                            })
                        },
                    }
                }).collect::<Vec<PathBuf>>()
            }
        }
    }
}*/

/*pub trait GetCommands {
    fn get_commands(&self, command: &str) -> Result<Vec<(String, String)>, Error>;
}

impl WorkspaceToml {
    fn excludes(&self) -> Vec<String> {
        match &self.exclude {
            None => vec![],
            Some(excludes) => excludes.to_vec(),
        }
    }

    pub fn members(&self) -> Vec<PathBuf> {
        match &self.members {
            None => vec![],
            Some(members) => {
                let excludes = self.excludes();
                extend_globs(&members).iter().filter_map(|member| {
                    match member {
                        Err(_) => None,
                        Ok(path) => {
                            path.to_str().map_or(None, |path_str| {
                                if excludes.contains(&String::from(path_str)) {
                                    Some(path.clone())
                                } else {
                                    None
                                }
                            })
                        },
                    }
                }).collect::<Vec<PathBuf>>()
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Package {
    #[serde(skip)]
    pub path: String,
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
            std::debug_assert!(cargo_toml.package.is_some());
            let mut package = cargo_toml.package.unwrap();
            package.path = cargo_toml.path;
            Ok(package)
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
    pub path: String,
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
            std::debug_assert!(cargo_toml.workspace.is_some());
            std::debug_assert!(cargo_toml.package.is_some());
            let workspace = Workspace::try_from(&cargo_toml)?;
            Ok(RootPackage {
                path: cargo_toml.path,
                workspace: workspace,
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
    pub members: Vec<Result<Package, Error>>,
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
            for member in self.members.iter() {
                if let Ok(package) = member {
                    let mut package_commands = package.get_commands(command)?;
                    commands.append(&mut package_commands);
                }
            }
            Ok(commands)
        } else {
            Ok(vec![])
        }
    }
}

impl TryFrom<WorkspaceToml> for Workspace {
    type Error = Error;

    fn try_from(cargo_toml: &'a CargoToml) -> Result<Self, Self::Error> {
        if cargo_toml.workspace.is_some() {
            std::debug_assert!(cargo_toml.workspace.is_some());
            let workspace = match cargo_toml.workspace {
                None => return Err(Error {
                    kind: ErrorKind::MissingWorkspace(cargo_toml.path),
                    message: String::new(),
                }),
                Some(workspace) => workspace,
            };
            let members = workspace.members().iter().map(|member| member.join("Cargo.toml")).collect::<Vec<PathBuf>>();
            let packages: Vec<Result<Package, Error>> = members.iter().map(|member| {
                member.to_str().map_or(Err(Error {
                    kind: ErrorKind::PathBufConversionError(format!("{:?}", member)),
                    message: String::from("Failed to convert path to string"),
                }), |path_str| {
                    let cargo_toml = CargoToml::from_path(path_str)?;
                    Package::try_from(cargo_toml)
                })
            }).collect();
            Ok(Workspace {
                members: packages,
                metadata: cargo_toml.workspace.map_or(None, |workspace| workspace.metadata),
            })
        } else {
            Err(Error {
                kind: ErrorKind::MissingWorkspace(cargo_toml.path),
                message: String::new(),
            })            
        }
    }
}

#[derive(Debug)]
pub struct VirtualManifest {
    pub path: String,
    pub members: Vec<Result<Package, Error>>,
    pub metadata: Option<Metadata>,
}

impl GetCommands for VirtualManifest {
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
            for member in self.members.iter() {
                if let Ok(package) = member {
                    let mut package_commands = package.get_commands(command)?;
                    commands.append(&mut package_commands);
                }
            }
            Ok(commands)
        } else {
            Ok(vec![])
        }
    }
}

impl TryFrom<CargoToml> for VirtualManifest {
    type Error = Error;

    fn try_from(cargo_toml: CargoToml) -> Result<Self, Self::Error> {
        if cargo_toml.is_virtual_manifest() {
            std::debug_assert!(cargo_toml.workspace.is_some());
            let workspace = match cargo_toml.workspace {
                None => return Err(Error {
                    kind: ErrorKind::MissingWorkspace(cargo_toml.path),
                    message: String::new(),
                }),
                Some(workspace) => workspace,
            };
            let excludes = workspace.exclude.unwrap_or(vec![]);
            let members = if let Some(members) = workspace.members {
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
            let packages: Vec<Result<Package, Error>> = members.iter().map(|member| {
                member.to_str().map_or(Err(Error {
                    kind: ErrorKind::PathBufConversionError(format!("{:?}", member)),
                    message: String::from("Failed to convert path to string"),
                }), |path_str| {
                    let cargo_toml = CargoToml::from_path(path_str)?;
                    Package::try_from(cargo_toml)
                })
            }).collect();
            Ok(VirtualManifest {
                path: cargo_toml.path,
                members: packages,
                metadata: workspace.metadata,
            })
        } else {
            Err(Error {
                kind: ErrorKind::NotVirtualManifest,
                message: String::new(),
            })            
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Metadata {
    commands: HashMap<String, String>,
}*/

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
    PatternError(String),
    GlobError(String),
    MissingCommand(String),
    MissingWorkspace(String),
    NotPackage,
    NotRootPackage,
    NotVirtualManifest,
    PathBufConversionError(String),
    MalformedManifest(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::IoError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::ParseError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::PatternError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::GlobError(reason) => write!(f, "{}: {}", self.message, reason)?,
            ErrorKind::MissingCommand(command) => write!(f, "Command \"{}\" not found in Cargo.toml", command)?,
            ErrorKind::MissingWorkspace(path) => write!(f, "Workspace not found in \"{}\"", path)?,
            ErrorKind::NotPackage => write!(f, "Cargo.toml does not contain a package")?,
            ErrorKind::NotRootPackage => write!(f, "Cargo.toml does not contain a root package")?,
            ErrorKind::NotVirtualManifest => write!(f, "Cargo.toml does not contain a virtual manifest")?,
            ErrorKind::PathBufConversionError(path) => write!(f, "{}: {}", self.message, path)?,
            ErrorKind::MalformedManifest(path) => write!(f, "Malformed manifest \"{}\": {}", path, self.message)?,
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

impl From<&toml::de::Error> for ErrorKind {
    fn from(error: &toml::de::Error) -> Self {
        ErrorKind::ParseError(format!("{}", error))
    }
}

impl From<glob::PatternError> for ErrorKind {
    fn from(error: glob::PatternError) -> Self {
        ErrorKind::PatternError(format!("{}", error))
    }
}

impl From<glob::GlobError> for ErrorKind {
    fn from(error: glob::GlobError) -> Self {
        ErrorKind::GlobError(format!("{}", error))
    }
}
