use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("User error: {0}")]
    User(String),
    
    #[error("Channel error: {0}")]
    Channel(String),
    
    #[error("Audio error: {0}")]
    Audio(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;