use serde::{Deserialize, Serialize};

/// A transaction to be relayed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Target contract address (checksummed hex).
    pub to: String,
    /// Encoded function calldata (hex with 0x prefix).
    pub data: String,
    /// Native token value to send (usually "0").
    pub value: String,
}

/// Wallet type for relayed transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayerTxType {
    /// Gnosis Safe wallet — must call deploy() before first tx.
    Safe,
    /// Proxy wallet — auto-deploys on first tx.
    Proxy,
}

impl RelayerTxType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelayerTxType::Safe => "SAFE",
            RelayerTxType::Proxy => "PROXY",
        }
    }
}

/// Transaction state in the relayer pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TxState {
    New,
    Executed,
    Mined,
    Confirmed,
    Failed,
    Invalid,
}

impl TxState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TxState::Confirmed | TxState::Failed | TxState::Invalid)
    }

    pub fn is_success(&self) -> bool {
        matches!(self, TxState::Mined | TxState::Confirmed)
    }
}

/// Result of a relayed transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayerTransactionResponse {
    #[serde(rename = "transactionID", alias = "transactionId")]
    pub transaction_id: String,
    pub state: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default, rename = "transactionHash")]
    pub transaction_hash: Option<String>,
}

/// Parsed transaction result.
#[derive(Debug, Clone)]
pub struct TxResult {
    pub state: TxState,
    pub tx_hash: Option<String>,
    pub proxy_address: Option<String>,
    pub error: Option<String>,
}

/// Signature parameters for Safe transactions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeSignatureParams {
    pub gas_price: String,
    pub operation: String,
    pub safe_txn_gas: String,
    pub base_gas: String,
    pub gas_token: String,
    pub refund_receiver: String,
}

impl Default for SafeSignatureParams {
    fn default() -> Self {
        Self {
            gas_price: "0".to_string(),
            operation: "0".to_string(),
            safe_txn_gas: "0".to_string(),
            base_gas: "0".to_string(),
            gas_token: "0x0000000000000000000000000000000000000000".to_string(),
            refund_receiver: "0x0000000000000000000000000000000000000000".to_string(),
        }
    }
}

/// Signature parameters for Proxy transactions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySignatureParams {
    pub gas_price: String,
    pub gas_limit: String,
    pub relayer_fee: String,
    pub relay_hub: String,
    pub relay: String,
}

/// Signature parameters for Safe-Create transactions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSignatureParams {
    pub payment_token: String,
    pub payment: String,
    pub payment_receiver: String,
}

impl Default for CreateSignatureParams {
    fn default() -> Self {
        Self {
            payment_token: "0x0000000000000000000000000000000000000000".to_string(),
            payment: "0".to_string(),
            payment_receiver: "0x0000000000000000000000000000000000000000".to_string(),
        }
    }
}

/// The full request body for POST /submit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    #[serde(rename = "type")]
    pub tx_type: String,
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_wallet: Option<String>,
    pub data: String,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    pub signature_params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Relay payload returned by GET /relay-payload.
#[derive(Debug, Clone, Deserialize)]
pub struct RelayPayload {
    pub address: String,
    pub nonce: String,
}
