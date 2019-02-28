pub use failure_derive::Fail;
pub use crate::util::error::{ErrorExt, ResultExt, ResultErrorExt};

pub type Error = crate::util::error::Error<ErrorId>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, Fail, PartialEq)]
pub enum ErrorId {
    #[fail(display = "IO error")]
    Io,

    #[fail(display = "Parse error")]
    Parse,

    #[fail(display = "Unknown error")]
    Unknown,
}