//! Redeem a single settled position (regular or neg-risk).
//!
//! Usage:
//!   cargo run --example redeem_single
//!
//! Required env vars (or .env file):
//!   PRIVATE_KEY=0x...
//!   POLY_RELAYER_API_KEY=...
//!   POLY_RELAYER_ADDRESS=0x...
//!   CONDITION_ID=0xabc123...
//!   NEG_RISK=false                 # set "true" for neg-risk markets

use ethers::signers::LocalWallet;
use polymarket_relayer::{
    operations, AuthMethod, RelayClient, RelayerTxType, Transaction,
};
use std::env;

fn require_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("Missing env var: {name} (set it in .env or export it)"))
}

fn parse_bytes32(hex_str: &str) -> anyhow::Result<[u8; 32]> {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(stripped)?;
    if bytes.len() != 32 {
        anyhow::bail!("CONDITION_ID must be 32 bytes, got {}", bytes.len());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let private_key = require_env("PRIVATE_KEY")?;
    let api_key = require_env("POLY_RELAYER_API_KEY")?;
    let address = require_env("POLY_RELAYER_ADDRESS")?;
    let condition_id_hex = require_env("CONDITION_ID")?;
    let is_neg_risk = env::var("NEG_RISK").unwrap_or_default() == "true";

    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &address);
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    let condition_id = parse_bytes32(&condition_id_hex)?;

    let tx: Transaction = if is_neg_risk {
        println!("Redeeming neg-risk position: {}", condition_id_hex);
        operations::redeem_neg_risk_positions(condition_id, &[1, 2])
    } else {
        println!("Redeeming regular position: {}", condition_id_hex);
        operations::redeem_regular(condition_id, &[1, 2])
    };

    let handle = client.execute(vec![tx], "Redeem settled position").await?;
    println!("Submitted tx: {}", handle.id());

    let result = handle.wait().await?;
    println!("Redeemed! tx: {}", result.tx_hash.unwrap_or_default());

    Ok(())
}
