//! Split USDC into outcome tokens, then merge them back.
//!
//! Usage:
//!   export PRIVATE_KEY="your_hex_private_key"
//!   export POLY_RELAYER_API_KEY="your_relayer_key"
//!   export POLY_RELAYER_ADDRESS="your_eoa_address"
//!   export CONDITION_ID="0xabc123..."
//!   export AMOUNT="1000000"              # 1 USDC (6 decimals)
//!   cargo run --example split_merge

use ethers::signers::LocalWallet;
use ethers::types::U256;
use polymarket_relayer::{operations, AuthMethod, RelayClient, RelayerTxType};
use std::env;

fn parse_bytes32(hex_str: &str) -> [u8; 32] {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(stripped).expect("invalid hex for condition_id");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let private_key = env::var("PRIVATE_KEY")?;
    let api_key = env::var("POLY_RELAYER_API_KEY")?;
    let address = env::var("POLY_RELAYER_ADDRESS")?;
    let condition_id_hex = env::var("CONDITION_ID")?;
    let amount_str = env::var("AMOUNT").unwrap_or("1000000".to_string()); // default 1 USDC

    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &address);
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    let condition_id = parse_bytes32(&condition_id_hex);
    let amount = U256::from_dec_str(&amount_str)?;
    let partition: Vec<u64> = vec![1, 2]; // binary outcome

    // ── Split: USDC → outcome tokens ──
    println!(
        "Splitting {} USDC into outcome tokens for condition {}...",
        amount, condition_id_hex
    );
    let split_tx = operations::split_regular(condition_id, &partition, amount);
    let handle = client.execute(vec![split_tx], "Split position").await?;
    let result = handle.wait().await?;
    println!(
        "✅ Split complete! tx: {}",
        result.tx_hash.unwrap_or_default()
    );

    // ── Merge: outcome tokens → USDC ──
    println!("\nMerging outcome tokens back to USDC...");
    let merge_tx = operations::merge_regular(condition_id, &partition, amount);
    let handle = client.execute(vec![merge_tx], "Merge positions").await?;
    let result = handle.wait().await?;
    println!(
        "✅ Merge complete! tx: {}",
        result.tx_hash.unwrap_or_default()
    );

    Ok(())
}
