# Build: polymarket-relayer-rs — Rust Builder Relayer Client

## What This Is

A Rust implementation of Polymarket's Builder Relayer Client — gasless on-chain operations (wallet deployment, token approvals, position redemption, transfers) without users paying gas.

This is the Rust equivalent of:
- Python: https://github.com/Polymarket/py-builder-relayer-client  
- TypeScript: https://github.com/Polymarket/builder-relayer-client
- Docs: https://docs.polymarket.com/trading/gasless

**This is NOT a CLOB/trading client.** It only handles on-chain operations relayed through Polymarket's gasless infrastructure.

## Why Build This

1. No Rust version exists. Only Python and TypeScript.
2. Multiple Rust bot developers in the Polymarket Discord need this — they can place orders via CLOB but can't do gasless redemption, approvals, or wallet deployment without switching to Python/TS.
3. The use case that gets the most complaints: **redeeming settled positions**. People win bets but can't easily claim their USDC programmatically in Rust.

## How The Relayer Works

1. Your app creates a transaction (e.g., "redeem this position")
2. User signs it with their private key
3. App sends the signed tx to Polymarket's relayer (`https://relayer-v2.polymarket.com/`)
4. Relayer submits it on-chain and pays the gas (POL)
5. Transaction executes from the user's wallet

## Authentication — Two Methods

### Method 1: Builder API Keys (HMAC-SHA256)
For Builder Program members. Requires:
- `POLY_BUILDER_API_KEY`
- `POLY_BUILDER_SECRET` 
- `POLY_BUILDER_PASSPHRASE`

Headers on every request:
```
POLY_BUILDER_API_KEY: {key}
POLY_BUILDER_TIMESTAMP: {unix_timestamp}
POLY_BUILDER_PASSPHRASE: {passphrase}
POLY_BUILDER_SIGNATURE: HMAC-SHA256(secret, timestamp + method + path + body)
```

### Method 2: Relayer API Keys (Simple)
For anyone. Created at polymarket.com/settings > API Keys. Headers:
```
RELAYER_API_KEY: {key}
RELAYER_API_KEY_ADDRESS: {owner_address}
```

**Support both methods.** Let user choose via config.

## Scope — What The SDK Must Do

### 1. RelayClient — Core

```rust
use polymarket_relayer::{RelayClient, RelayerTxType, BuilderConfig, Transaction};

// Initialize with Builder API keys
let config = BuilderConfig::local(
    "your_api_key",
    "your_secret", 
    "your_passphrase",
);

let client = RelayClient::new(
    "https://relayer-v2.polymarket.com/",
    137,  // Polygon chain ID
    signer,  // ethers LocalWallet or equivalent
    config,
    RelayerTxType::Safe,  // or Proxy
).await?;

// Deploy wallet (one-time, Safe wallets only)
let result = client.deploy().await?;
println!("Safe address: {}", result.proxy_address);

// Execute gasless transaction
let tx = Transaction {
    to: contract_address,
    data: encoded_calldata,
    value: "0".to_string(),
};
let response = client.execute(vec![tx], "Redeem positions").await?;
let result = response.wait().await?;
```

### 2. Token Approvals

```rust
use polymarket_relayer::operations;

// Approve USDC.e for CTF Exchange (one-time setup)
client.approve_usdc_for_ctf().await?;

// Approve USDC.e for NegRisk Exchange  
client.approve_usdc_for_negrisk().await?;

// Approve all outcome tokens
client.approve_ctf_for_exchange().await?;

// Or custom approval
let tx = operations::approve(
    usdc_address,
    spender_address,
    u256::MAX,
);
client.execute(vec![tx], "Custom approval").await?;
```

### 3. Redeem Positions (Main Use Case)

```rust
use polymarket_relayer::operations;

// Redeem a regular (non-neg-risk) settled position
let tx = operations::redeem_positions(
    collateral_token,   // USDC.e address
    parent_collection,  // bytes32, usually 0x00...00
    condition_id,       // from market data
    vec![1, 2],         // index sets (both outcomes)
);
client.execute(vec![tx], "Redeem regular position").await?;

// Redeem a neg-risk settled position
let tx = operations::redeem_neg_risk(
    condition_id,
    vec![1, 2],
);
client.execute(vec![tx], "Redeem neg-risk position").await?;

// High-level: redeem ALL settled positions for a wallet
let redeemed = client.redeem_all_settled().await?;
for r in &redeemed {
    println!("Redeemed: {} | +${:.2}", r.market_title, r.usdc_amount);
}
```

### 4. CTF Operations (Split / Merge)

```rust
// Split USDC into outcome tokens
let tx = operations::split_position(
    collateral_token,
    parent_collection,
    condition_id,
    partition,       // [1, 2]
    amount,          // USDC amount (6 decimals)
);
client.execute(vec![tx], "Split position").await?;

// Merge outcome tokens back to USDC
let tx = operations::merge_positions(
    collateral_token,
    parent_collection,
    condition_id,
    partition,
    amount,
);
client.execute(vec![tx], "Merge positions").await?;
```

### 5. Batch Transactions

```rust
// Multiple operations in one atomic call
let approve_tx = operations::approve(usdc, ctf, u256::MAX);
let redeem_tx = operations::redeem_positions(usdc, parent, cond_id, vec![1, 2]);

// Both succeed or both fail
client.execute(vec![approve_tx, redeem_tx], "Approve + Redeem").await?;
```

### 6. Transaction Status Tracking

```rust
let response = client.execute(txs, "description").await?;

// Poll until terminal state
let result = response.wait().await?;

match result.state {
    TxState::Confirmed => println!("Success! tx: {}", result.tx_hash),
    TxState::Failed => println!("Failed: {}", result.error),
    TxState::Invalid => println!("Rejected: {}", result.error),
    _ => {}
}

// Or check manually
let status = client.get_transaction(tx_id).await?;
```

## Types

```rust
pub enum RelayerTxType {
    Safe,   // Call deploy() before first tx
    Proxy,  // Auto-deploys on first tx
}

pub enum AuthMethod {
    Builder(BuilderConfig),
    RelayerKey { api_key: String, address: String },
}

pub struct BuilderConfig {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

pub struct Transaction {
    pub to: String,      // Target contract address
    pub data: String,     // Encoded function calldata (hex)
    pub value: String,    // POL to send (usually "0")
}

pub enum TxState {
    New,        // Received by relayer
    Executed,   // Submitted on-chain
    Mined,      // Included in block
    Confirmed,  // Finalized ✓
    Failed,     // Failed permanently ✗
    Invalid,    // Rejected ✗
}

pub struct TxResult {
    pub state: TxState,
    pub tx_hash: Option<String>,
    pub proxy_address: Option<String>,
    pub error: Option<String>,
}

pub struct RedeemResult {
    pub condition_id: String,
    pub market_title: String,
    pub usdc_amount: f64,
    pub tx_hash: String,
}
```

## Contract Addresses (Polygon Mainnet)

```rust
pub const USDC_E: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
pub const CTF: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
pub const CTF_EXCHANGE: &str = "0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e";
pub const NEG_RISK_EXCHANGE: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";
pub const NEG_RISK_ADAPTER: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";
pub const PROXY_FACTORY: &str = "0xaB45c5A4B0c941a2F231C04C3f49182e1A254052";
pub const RELAYER_URL: &str = "https://relayer-v2.polymarket.com/";
```

## File Structure

```
polymarket-relayer-rs/
├── Cargo.toml
├── README.md
├── examples/
│   ├── setup_wallet.rs         # Deploy Safe + set approvals
│   ├── redeem_single.rs        # Redeem one settled position
│   ├── redeem_all.rs           # Redeem all settled positions
│   └── split_merge.rs          # Split and merge demo
├── src/
│   ├── lib.rs
│   ├── client.rs               # RelayClient core
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── builder.rs          # Builder HMAC-SHA256 signing
│   │   └── relayer_key.rs      # Simple Relayer API key auth
│   ├── operations/
│   │   ├── mod.rs
│   │   ├── approve.rs          # Token approval helpers
│   │   ├── redeem.rs           # Redeem position helpers
│   │   ├── split_merge.rs      # Split/merge helpers
│   │   └── deploy.rs           # Wallet deployment
│   ├── types.rs                # Transaction, TxState, etc.
│   ├── contracts.rs            # ABIs and addresses
│   └── error.rs                # Custom error types
└── tests/
    ├── auth_test.rs
    └── integration_test.rs
```

## Dependencies

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ethers = { version = "2", features = ["abigen"] }
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
thiserror = "1"
tracing = "0.1"
```

## Implementation Priority

1. **Auth module** — Builder HMAC signing + Relayer key auth (both methods)
2. **RelayClient.execute()** — Core transaction submission + status polling
3. **operations::redeem** — This is what people need most urgently
4. **operations::approve** — One-time setup helpers
5. **redeem_all example** — The killer demo that shows the value
6. **README** — Quick start in 5 lines of code
7. Split/merge, deploy, batch — Nice to have

## README Quick Start (Target)

```rust
use polymarket_relayer::{RelayClient, AuthMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = RelayClient::new(
        AuthMethod::relayer_key("your_key", "your_address"),
        "your_private_key",
    ).await?;
    
    // Redeem all your settled positions in one call
    let results = client.redeem_all_settled().await?;
    for r in &results {
        println!("✅ {} | +${:.2}", r.market_title, r.usdc_amount);
    }
    Ok(())
}
```

That's it. 10 lines to claim all your winnings. No gas needed.

## Reference

- Python SDK: https://github.com/Polymarket/py-builder-relayer-client
- TS SDK: https://github.com/Polymarket/builder-relayer-client
- Signing SDK (Python): https://github.com/Polymarket/py-builder-signing-sdk
- Signing SDK (TS): https://github.com/Polymarket/builder-signing-sdk
- Docs: https://docs.polymarket.com/trading/gasless
- Contract addresses: https://docs.polymarket.com/resources/contract-addresses