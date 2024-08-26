use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to start the game: {0}")]
    StartGameError(String),

    #[error("Internal game error: {0}")]
    InternalGameError(String),

    #[error("Invalid transaction: {0}")]
    InvalidTransactionError(String),

    #[error("Block validation failed: {0}")]
    BlockValidationError(String),

    #[error("No leader found")]
    NoLeaderError,

    #[error("Quorum certificate invalid")]
    InvalidQcError,

    #[error("gRPC server error: {0}")]
    GrpcServerError(String),

    #[error("Peer error: {0}")]
    PeerError(String),

    #[error("Swarm error: {0}")]
    SwarmError(String),

    #[error("Unknown error")]
    UnknownError,
}
