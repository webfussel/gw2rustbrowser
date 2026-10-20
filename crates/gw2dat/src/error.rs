use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("not a valid Gw2.dat: {0}")]
    InvalidArchive(String),
    #[error("MFT entry {0} does not exist")]
    NoSuchEntry(u32),
    #[error("MFT entry {0} is not in use")]
    UnusedEntry(u32),
    #[error("no file with id {0}")]
    NoSuchId(u32),
    /// The input ended before the decoder was done. When peeking this means
    /// "read more input and try again"; on a full read it means corrupt data.
    #[error("input data is truncated")]
    Truncated,
    #[error("decompression failed: {0}")]
    Decompress(&'static str),
    #[error("invalid {what}: {reason}")]
    Format { what: &'static str, reason: String },
    #[error("unsupported {0}")]
    Unsupported(String),
}

impl Error {
    pub(crate) fn format(what: &'static str, reason: impl Into<String>) -> Self {
        Error::Format { what, reason: reason.into() }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
