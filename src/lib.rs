pub mod auth;
pub mod builder;
pub mod client;
pub mod contracts;
pub mod direct;
pub mod error;
pub mod operations;
pub mod types;

// Re-export key types for convenience.
pub use auth::{AuthMethod, BuilderConfig};
pub use client::{RelayClient, TransactionResponseHandle};
pub use direct::{DirectExecutor, DirectTxResult};
pub use error::{RelayerError, Result};
pub use operations::{
    approve, approve_ctf_for_ctf_exchange, approve_ctf_for_neg_risk_adapter,
    approve_ctf_for_neg_risk_exchange, approve_usdc_for_ctf_exchange,
    approve_usdc_for_neg_risk_exchange, merge_positions, merge_regular, redeem_neg_risk_positions,
    redeem_positions, redeem_regular, set_approval_for_all, split_position, split_regular,
};
pub use types::{RelayerTxType, Transaction, TxResult, TxState};
