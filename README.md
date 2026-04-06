# polymarket-relayer

**Gasless on-chain operations for Polymarket — in Rust.**

Claim your winnings, approve tokens, split/merge positions — all without paying gas. This is the Rust SDK for [Polymarket's Builder Relayer](https://docs.polymarket.com/trading/gasless).

```toml
[dependencies]
polymarket-relayer = "0.1"
polymarket-client-sdk = { version = "0.4", features = ["data"] }
ethers = { version = "2", features = ["abigen"] }
tokio = { version = "1", features = ["full"] }
```

---

## Quick Start — Scan & Redeem All Settled Positions

```rust
use ethers::signers::LocalWallet;
use polymarket_client_sdk::data::Client as DataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_relayer::{operations, AuthMethod, RelayClient, RelayerTxType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wallet: LocalWallet = std::env::var("PRIVATE_KEY")?.parse()?;
    let client = RelayClient::new(
        137,
        wallet,
        AuthMethod::relayer_key(
            &std::env::var("POLY_RELAYER_API_KEY")?,
            &std::env::var("POLY_RELAYER_ADDRESS")?,
        ),
        RelayerTxType::Safe,
    ).await?;

    // Step 1: Query all your positions from the Polymarket Data API
    let data = DataClient::default();
    let positions = data.positions(
        &PositionsRequest::builder()
            .user(std::env::var("POLY_RELAYER_ADDRESS")?.parse()?)
            .limit(500)?
            .build(),
    ).await?;

    // Step 2: Filter settled positions, build redeem txs
    let mut seen = std::collections::HashSet::new();
    for pos in positions.iter().filter(|p| p.redeemable) {
        let cid = pos.condition_id;
        if !seen.insert(cid) { continue; }  // one redeem per condition_id

        let tx = if pos.negative_risk {
            operations::redeem_neg_risk_positions(cid, &[1, 2])
        } else {
            operations::redeem_regular(cid, &[1, 2])
        };

        // Step 3: Redeem gaslessly through the relayer
        let result = client.execute(vec![tx], &pos.title).await?.wait().await?;
        println!("{} | tx: {}", pos.title, result.tx_hash.unwrap_or_default());
    }

    Ok(())
}
```

That's it. Scan your positions, redeem them all, pay zero gas.

Get your **Relayer API key** at [polymarket.com/settings](https://polymarket.com/settings) > API Keys.

---

## One-Time Setup (Safe Wallet)

Before your first transaction, deploy your Safe and approve tokens:

```rust
// Deploy Safe wallet (once only)
client.deploy().await?;

// Approve USDC + outcome tokens for all exchanges (once only)
client.setup_approvals().await?.wait().await?;
```

---

## What This SDK Does

| Feature | Method |
|---|---|
| Deploy Safe wallet | `client.deploy()` |
| Set all token approvals | `client.setup_approvals()` |
| Redeem regular position | `operations::redeem_regular(condition_id, &[1, 2])` |
| Redeem neg-risk position | `operations::redeem_neg_risk_positions(condition_id, &[1, 2])` |
| Split USDC into outcome tokens | `operations::split_regular(condition_id, &[1, 2], amount)` |
| Merge outcome tokens back to USDC | `operations::merge_regular(condition_id, &[1, 2], amount)` |
| Approve USDC for CTF Exchange | `operations::approve_usdc_for_ctf_exchange()` |
| Batch multiple operations | `client.execute(vec![tx1, tx2, tx3], "desc")` |

---

## Authentication

### Option A: Relayer API Key (recommended for most users)

Get your key at [polymarket.com/settings](https://polymarket.com/settings) > API Keys.

```rust
AuthMethod::relayer_key("your_api_key", "your_eoa_address")
```

### Option B: Builder API Key (for Builder Program members)

```rust
AuthMethod::builder("api_key", "secret", "passphrase")
```

---

## Wallet Types

| Type | Notes |
|---|---|
| `RelayerTxType::Safe` | Gnosis Safe. Requires `client.deploy()` before first use. |
| `RelayerTxType::Proxy` | Lighter proxy wallet. Auto-deploys on first tx. |

---

## Full Examples

```bash
# 1. Deploy Safe + set approvals
PRIVATE_KEY=0x... POLY_RELAYER_API_KEY=... POLY_RELAYER_ADDRESS=0x... \
  cargo run --example setup_wallet

# 2. Redeem a specific position
CONDITION_ID=0xabc... cargo run --example redeem_single

# 3. Scan ALL positions + redeem everything settled (dry-run first)
cargo run --example redeem_all
cargo run --example redeem_all -- --execute

# 4. Split and merge positions
CONDITION_ID=0xabc... AMOUNT=1000000 cargo run --example split_merge
```

---

## Contract Addresses (Polygon Mainnet)

| Contract | Address |
|---|---|
| USDC.e | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` |
| CTF | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` |
| CTF Exchange | `0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E` |
| NegRisk Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` |
| NegRisk Adapter | `0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296` |
| Relayer | `https://relayer-v2.polymarket.com/` |

---

## References

- [Polymarket Gasless Docs](https://docs.polymarket.com/trading/gasless)
- [Python SDK](https://github.com/Polymarket/py-builder-relayer-client)
- [TypeScript SDK](https://github.com/Polymarket/builder-relayer-client)
- [Contract Addresses](https://docs.polymarket.com/resources/contract-addresses)

---

## Donate

If this SDK saved you time, consider buying me a coffee:

**Ethereum / Polygon**
```
0xF4c6635dFfB53f21c500c1604EC284f8A8a7150D
```
