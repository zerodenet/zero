use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Config(&'static str),
    Io(&'static str),
    Protocol(&'static str),
    Route(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(f, "config error: {message}"),
            Self::Io(message) => write!(f, "io error: {message}"),
            Self::Protocol(message) => write!(f, "protocol error: {message}"),
            Self::Route(message) => write!(f, "route error: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported: {message}"),
        }
    }
}

impl core::error::Error for Error {}
