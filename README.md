# rs-builder-relayer-client

Rust SDK for [Polymarket's gasless relayer](https://docs.polymarket.com/trading/gasless). Redeem positions, approve tokens, split/merge — zero gas.

## Install

```toml
[dependencies]
rs-builder-relayer-client = "0.1"
ethers = "2"
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use polymarket_relayer::{operations, AuthMethod, RelayClient, RelayerTxType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wallet = std::env::var("PRIVATE_KEY")?.parse()?;
    let client = RelayClient::new(
        137,
        wallet,
        AuthMethod::builder("key", "secret", "passphrase"),
        RelayerTxType::Safe,
    ).await?;

    // Redeem a settled position (gasless)
    let condition_id = hex::decode("ab12...cd34")?;  // 32 bytes
    let mut cid = [0u8; 32];
    cid.copy_from_slice(&condition_id);

    let tx = operations::redeem_regular(cid, &[1, 2]);
    let result = client.execute(vec![tx], "Redeem").await?.wait().await?;
    println!("tx: {}", result.tx_hash.unwrap_or_default());

    Ok(())
}
```

## API

| Operation | Code |
|---|---|
| Redeem regular position | `operations::redeem_regular(condition_id, &[1, 2])` |
| Redeem neg-risk position | `operations::redeem_neg_risk_positions(condition_id, &[1, 2])` |
| Approve USDC for exchange | `client.setup_approvals()` |
| Deploy Safe wallet | `client.deploy()` |
| Split USDC into tokens | `operations::split_regular(cid, &[1, 2], amount)` |
| Merge tokens back to USDC | `operations::merge_regular(cid, &[1, 2], amount)` |
| Batch multiple ops | `client.execute(vec![tx1, tx2], "desc")` |
| Direct on-chain fallback | `DirectExecutor::new(rpc_url, wallet, 137)?` |

## Auth

```rust
// Builder API keys (HMAC — enables gasless)
AuthMethod::builder("key", "secret", "passphrase")

// Relayer API keys (from polymarket.com/settings > API Keys)
AuthMethod::relayer_key("api_key", "wallet_address")
```

## Direct Fallback (when relayer returns 429)

```rust
use polymarket_relayer::{DirectExecutor, RelayerError};

let direct = DirectExecutor::new("https://polygon-rpc.com", wallet, 137)?;

match client.execute(vec![tx], "Redeem").await {
    Err(RelayerError::QuotaExhausted) => {
        let result = direct.execute(&tx).await?;  // pays gas in MATIC
    }
    other => { /* handle normally */ }
}
```

## Examples

```bash
cp .env.example .env   # fill in your keys

cargo run --example redeem_all                  # dry-run: scan positions
cargo run --example redeem_all -- --execute     # actually redeem
cargo run --example setup_wallet                # deploy Safe + approvals
cargo run --example redeem_single               # redeem one position
cargo run --example split_merge                 # split/merge demo
```

## References

- [Gasless Docs](https://docs.polymarket.com/trading/gasless) | [Python SDK](https://github.com/Polymarket/py-builder-relayer-client) | [TypeScript SDK](https://github.com/Polymarket/builder-relayer-client)

## Donate

**Ethereum / Polygon:** `0xF4c6635dFfB53f21c500c1604EC284f8A8a7150D`
