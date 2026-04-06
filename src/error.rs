use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Relayer API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("ABI encoding error: {0}")]
    Abi(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Transaction invalid: {0}")]
    TransactionInvalid(String),

    #[error("Timeout waiting for transaction confirmation")]
    Timeout,

    #[error("Wallet not deployed: {0}")]
    WalletNotDeployed(String),

    #[error("Wallet already deployed: {0}")]
    WalletAlreadyDeployed(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),

    #[error("Auth error: {0}")]
    AuthError(String),

    #[error("Relayer quota exhausted (429)")]
    QuotaExhausted,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RelayerError>;
