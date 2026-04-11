# rs-builder-relayer-client

Rust SDK for [Polymarket's gasless relayer](https://docs.polymarket.com/trading/gasless). Redeem positions, approve tokens, split/merge — zero gas.

## 30-Second Quickstart

```bash
cargo new my-redeemer && cd my-redeemer
cargo add rs-builder-relayer-client ethers tokio --features tokio/full
cargo add anyhow dotenvy hex
```

Create `.env`:
```
PRIVATE_KEY=0x...
BUILDER_KEY=...
BUILDER_SECRET=...
BUILDER_PASSPHRASE=...
# Optional: Use Alchemy or QuickNode for Direct Fallback. Default polygon-rpc.com is unstable.
POLYGON_RPC_URL=https://...
```

`src/main.rs`:
```rust
use polymarket_relayer::{RelayClient, AuthMethod, RelayerTxType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let wallet = std::env::var("PRIVATE_KEY")?.parse()?;
    let mut client = RelayClient::new(
        137, wallet,
        AuthMethod::builder(
            &std::env::var("BUILDER_KEY")?,
            &std::env::var("BUILDER_SECRET")?,
            &std::env::var("BUILDER_PASSPHRASE")?,
        ),
        RelayerTxType::Safe,
    ).await?;

    // Read nonce from on-chain (avoids stale relayer API nonce → GS026)
    if let Ok(rpc) = std::env::var("POLYGON_RPC_URL") {
        client.set_rpc_url(rpc);
    }

    client.setup_approvals().await?.wait().await?;
    println!("Done. You can now trade gaslessly.");
    Ok(())
}
```

```bash
cargo run
```

## Getting Your Credentials

| Credential | Where |
|---|---|
| `PRIVATE_KEY` | Your Polygon wallet private key (MetaMask > Account Details > Export) |
| Relayer API key | [polymarket.com/settings > Relayer API Keys](https://polymarket.com/settings) (anyone) |

No Builder keys? Use `AuthMethod::relayer_key("key", "address")` instead — same features, simpler setup.

---

## Install

```toml
[dependencies]
rs-builder-relayer-client = "0.1"
ethers = "2"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
dotenvy = "0.15"
hex = "0.4"
```

## Redeem Example

Add `CONDITION_ID=0x...` to your `.env`, then:

```rust
use polymarket_relayer::{AuthMethod, RelayClient, RelayerTxType, operations};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let wallet = std::env::var("PRIVATE_KEY")?.parse()?;
    let client = RelayClient::new(
        137,
        wallet,
        AuthMethod::builder(
            &std::env::var("BUILDER_KEY")?,
            &std::env::var("BUILDER_SECRET")?,
            &std::env::var("BUILDER_PASSPHRASE")?,
        ),
        RelayerTxType::Safe,
    ).await?;

    let condition_id_hex = std::env::var("CONDITION_ID")?;
    let condition_id_bytes = hex::decode(condition_id_hex.trim_start_matches("0x"))?;
    let mut cid = [0u8; 32];
    cid.copy_from_slice(&condition_id_bytes);

    let tx = operations::redeem_regular(cid, &[1, 2]);
    let result = client.execute(vec![tx], "Redeem").await?.wait().await?;
    println!("Transaction Hash: {:?}", result.tx_hash);

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
| Execute single/multiple ops | `client.execute(vec![tx1], "desc")` |
| Execute true multi-send batch | `client.execute_batch(vec![tx1, tx2], "desc")` |
| Execute chunks sequentially | `client.execute_sequential(vec![vec![tx1], vec![tx2]], None, None)` |
| Direct on-chain fallback | `DirectExecutor::new(rpc_url, wallet, 137)?` |

## Auth

```rust
// Builder API keys (HMAC — enables gasless)
AuthMethod::builder("key", "secret", "passphrase")

// Relayer API keys (from polymarket.com/settings > API Keys)
AuthMethod::relayer_key("api_key", "wallet_address")
```

## Direct Fallback (when relayer returns 429)

> **Warning:** Do **not** use `https://polygon-rpc.com/` as your RPC URL — it frequently causes TLS handshake EOF errors and connection resets, especially under load. Use a dedicated provider instead:
> - [Alchemy](https://www.alchemy.com/) (recommended): `https://polygon-mainnet.g.alchemy.com/v2/YOUR_KEY`
> - [QuickNode](https://www.quicknode.com/): `https://YOUR_ENDPOINT.quiknode.pro/YOUR_KEY/`
> - [LlamaRPC](https://llamarpc.com/): `https://polygon.llamarpc.com`

```rust
use polymarket_relayer::{DirectExecutor, RelayerError};

let rpc_url = std::env::var("POLYGON_RPC_URL")
    .expect("Set POLYGON_RPC_URL to an Alchemy/QuickNode endpoint");

// Safe wallet (signature_type=2, default)
let direct = DirectExecutor::new(&rpc_url, wallet, 137)?;

// Proxy wallet (signature_type=1, e.g. magic.link)
let direct = DirectExecutor::new_proxy(&rpc_url, wallet, 137)?;

// Proxy with explicit address (when derived address differs)
let direct = DirectExecutor::new_proxy_with_address(&rpc_url, wallet, 137, proxy_addr)?;

match client.execute(vec![tx], "Redeem").await {
    Err(RelayerError::QuotaExhausted) => {
        let result = direct.execute(&tx).await?;  // pays gas in MATIC
    }
    other => { /* handle normally */ }
}
```

## Batching & Execution Strategies

Depending on whether you use `RelayerTxType::Safe` or `RelayerTxType::Proxy`, the SDK provides several execution models:

* **`client.execute` / `client.execute_batch`**:
  * **Safe Wallets**: Uses official Gnosis `MultiSend` contracts. Multiple operations are packed tightly into a single transaction. Safe is highly durable and recommended for heavy batching (> 2 operations).
  * **Proxy Wallets**: While the OpenGSN proxy supports a `(uint8, address, uint256, bytes)[]` array structure, the Polymarket relayer bot imposes strict total-transaction gas limits top-level. **Batching more than 2 operations with Proxy wallets is highly discouraged** and might hit silent `relay hub: internal transaction failure` errors due to gas starvation. The SDK dynamically scales requests up to a hard cap of 400K gas. 

* **`client.execute_sequential`**: 
  Designed purely to circumvent Proxy Relayer bottlenecks when you have e.g. 10 positions to redeem. It executes batches step-by-step, patiently awaiting `STATE_CONFIRMED` to prevent nonce collisions and OpenGSN RelayHub deadlocks across Gelato's relayer pools.

## Examples

```bash
cp .env.example .env   # fill in your keys

cargo run --example redeem_all                  # dry-run: scan positions
cargo run --example redeem_all -- --execute     # actually redeem
cargo run --example setup_wallet                # deploy Safe + approvals
cargo run --example redeem_single               # redeem one position
cargo run --example split_merge                 # split/merge demo
cargo run --example redeem_magic                # magic.link proxy wallet redeem
```

## References

- [Gasless Docs](https://docs.polymarket.com/trading/gasless) | [Python SDK](https://github.com/Polymarket/py-builder-relayer-client) | [TypeScript SDK](https://github.com/Polymarket/builder-relayer-client)

## Donate

**Ethereum / Polygon:** `0xF4c6635dFfB53f21c500c1604EC284f8A8a7150D`
