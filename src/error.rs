use std::io;

use value::ValueError;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    ValueError(ValueError),
    Generic(&'static str),
}

impl From<io::Error> for Error {
    fn from(v: io::Error) -> Self {
        Error::Io(v)
    }
}

impl From<ValueError> for Error {
    fn from(v: ValueError) -> Self {
        Error::ValueError(v)
    }
}

impl From<&'static str> for Error {
    fn from(v: &'static str) -> Self {
        Error::Generic(v)
    }
}