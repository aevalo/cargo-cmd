use std::fmt;
use std::io;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum ErrorKind {
    IoError(String),
    ParseError(String),
    PatternError(String),
    GlobError(String),
    MissingCommand(String),
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
