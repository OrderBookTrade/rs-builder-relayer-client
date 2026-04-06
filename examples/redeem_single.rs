//! Redeem a single settled position (regular or neg-risk).
//!
//! Usage:
//!   export PRIVATE_KEY="your_hex_private_key"
//!   export POLY_RELAYER_API_KEY="your_relayer_key"
//!   export POLY_RELAYER_ADDRESS="your_eoa_address"
//!   export CONDITION_ID="0xabc123..."    # 32-byte hex condition ID
//!   export NEG_RISK="false"              # set "true" for neg-risk markets
//!   cargo run --example redeem_single

use ethers::signers::LocalWallet;
use polymarket_relayer::{
    operations, AuthMethod, RelayClient, RelayerTxType, Transaction,
};
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
    let is_neg_risk = env::var("NEG_RISK").unwrap_or_default() == "true";

    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &address);
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    let condition_id = parse_bytes32(&condition_id_hex);
    let index_sets: Vec<u64> = vec![1, 2]; // both outcomes

    // Build the right redeem transaction
    let tx: Transaction = if is_neg_risk {
        println!("Redeeming neg-risk position: {}", condition_id_hex);
        operations::redeem_neg_risk_positions(condition_id, &index_sets)
    } else {
        println!("Redeeming regular position: {}", condition_id_hex);
        operations::redeem_regular(condition_id, &index_sets)
    };

    let handle = client.execute(vec![tx], "Redeem settled position").await?;
    println!("Submitted tx: {}", handle.id());

    let result = handle.wait().await?;
    println!(
        "✅ Redeemed! tx: {}",
        result.tx_hash.unwrap_or_default()
    );

    Ok(())
}
