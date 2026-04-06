# polymarket-relayer

**Gasless on-chain operations for Polymarket — in Rust.**

Claim your winnings, approve tokens, split/merge positions — all without paying gas. This is the Rust SDK for [Polymarket's Builder Relayer](https://docs.polymarket.com/trading/gasless).

```toml
[dependencies]
polymarket-relayer = { git = "https://github.com/youruser/rs-builder-relayer-client" }
```

---

## Quick Start — Claim All Settled Positions

```rust
use polymarket_relayer::{RelayClient, AuthMethod, RelayerTxType, operations};
use ethers::signers::LocalWallet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wallet: LocalWallet = std::env::var("PRIVATE_KEY")?.parse()?;

    let client = RelayClient::new(
        137,  // Polygon mainnet
        wallet,
        AuthMethod::relayer_key(
            &std::env::var("POLY_RELAYER_API_KEY")?,
            &std::env::var("POLY_RELAYER_ADDRESS")?,
        ),
        RelayerTxType::Safe,
    ).await?;

    // Get your settled positions from the CLOB API, then redeem them all in one batch:
    let condition_ids: Vec<[u8; 32]> = vec![/* from API */];

    let txs: Vec<_> = condition_ids.iter()
        .map(|&id| operations::redeem_regular(id, &[1, 2]))
        .collect();

    let result = client.execute(txs, "Redeem all").await?.wait().await?;
    println!("✅ Claimed! tx: {}", result.tx_hash.unwrap_or_default());

    Ok(())
}
```

Get your **Relayer API key** at [polymarket.com/settings](https://polymarket.com/settings) → API Keys.

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
| Split USDC → outcome tokens | `operations::split_regular(condition_id, &[1, 2], amount)` |
| Merge outcome tokens → USDC | `operations::merge_regular(condition_id, &[1, 2], amount)` |
| Approve USDC for CTF Exchange | `operations::approve_usdc_for_ctf_exchange()` |
| Batch multiple operations | `client.execute(vec![tx1, tx2, tx3], "desc")` |

---

## Authentication

### Option A: Relayer API Key (recommended for most users)

Get your key at [polymarket.com/settings](https://polymarket.com/settings) → API Keys.

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

# 3. Batch redeem multiple positions
cargo run --example redeem_all

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
