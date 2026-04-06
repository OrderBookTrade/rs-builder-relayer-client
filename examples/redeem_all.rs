//! Redeem multiple settled positions at once (batch).
//!
//! This example demonstrates building multiple redeem transactions
//! and submitting them as a single atomic batch.
//!
//! Usage:
//!   export PRIVATE_KEY="your_hex_private_key"
//!   export POLY_RELAYER_API_KEY="your_relayer_key"
//!   export POLY_RELAYER_ADDRESS="your_eoa_address"
//!   cargo run --example redeem_all

use ethers::signers::LocalWallet;
use polymarket_relayer::{operations, AuthMethod, RelayClient, RelayerTxType};
use std::env;

/// Example settled positions to redeem.
/// In production you'd fetch these from the Polymarket CLOB API or your own DB.
struct SettledPosition {
    condition_id: [u8; 32],
    is_neg_risk: bool,
    title: String,
}

fn example_positions() -> Vec<SettledPosition> {
    // These are dummy condition IDs for demonstration.
    // Replace with real values from your settled positions.
    vec![
        SettledPosition {
            condition_id: [0xAA; 32],
            is_neg_risk: false,
            title: "Will BTC reach $100k by EOY?".to_string(),
        },
        SettledPosition {
            condition_id: [0xBB; 32],
            is_neg_risk: true,
            title: "Presidential Election 2024 — Winner".to_string(),
        },
    ]
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let private_key = env::var("PRIVATE_KEY")?;
    let api_key = env::var("POLY_RELAYER_API_KEY")?;
    let address = env::var("POLY_RELAYER_ADDRESS")?;

    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &address);
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    let positions = example_positions();
    let index_sets: Vec<u64> = vec![1, 2];

    // Build one Transaction per settled position
    let txs: Vec<_> = positions
        .iter()
        .map(|pos| {
            if pos.is_neg_risk {
                operations::redeem_neg_risk_positions(pos.condition_id, &index_sets)
            } else {
                operations::redeem_regular(pos.condition_id, &index_sets)
            }
        })
        .collect();

    println!("Redeeming {} positions in a single batch:", txs.len());
    for pos in &positions {
        println!("  • {}", pos.title);
    }

    let handle = client
        .execute(txs, "Batch redeem all settled positions")
        .await?;
    println!("\nSubmitted tx: {}", handle.id());

    let result = handle.wait().await?;
    println!(
        "✅ All redeemed! tx: {}",
        result.tx_hash.unwrap_or_default()
    );

    Ok(())
}
