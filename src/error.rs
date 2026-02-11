use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaryaError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(String),

    #[error("PTY error: {0}")]
    Pty(String),
}

pub type Result<T> = std::result::Result<T, DaryaError>;
