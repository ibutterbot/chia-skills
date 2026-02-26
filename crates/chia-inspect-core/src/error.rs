use thiserror::Error;

#[derive(Debug, Error)]
pub enum InspectError {
    #[error("invalid input shape: {0}")]
    InvalidInput(String),
}
