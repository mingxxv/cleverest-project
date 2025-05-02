use thiserror::Error;

#[derive(Error, Debug)]
pub enum CleverestError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Capture error: {0}")]
    Capture(String),
    
    #[error("Display error: {0}")]
    Display(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    
    #[error("Client error: {0}")]
    Client(String),
    
    #[error("Server error: {0}")]
    Server(String),

    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, CleverestError>;