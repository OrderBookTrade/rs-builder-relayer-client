//! Deploy a Safe wallet and set up all standard token approvals.
//!
//! Usage:
//!   export PRIVATE_KEY="your_hex_private_key"
//!   export POLY_RELAYER_API_KEY="your_relayer_key"
//!   export POLY_RELAYER_ADDRESS="your_eoa_address"
//!   cargo run --example setup_wallet

use ethers::signers::LocalWallet;
use polymarket_relayer::{AuthMethod, RelayClient, RelayerTxType};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 1. Load credentials from env ──
    let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY env var required");
    let api_key = env::var("POLY_RELAYER_API_KEY").expect("POLY_RELAYER_API_KEY env var required");
    let address = env::var("POLY_RELAYER_ADDRESS").expect("POLY_RELAYER_ADDRESS env var required");

    // ── 2. Build the client ──
    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &address);

    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    let safe_addr = client.wallet_address()?;
    println!("EOA:  {:?}", client.signer_address());
    println!("Safe: {:?}", safe_addr);

    // ── 3. Deploy Safe if needed ──
    match client.deploy().await {
        Ok(result) => {
            println!(
                "✅ Safe deployed! tx: {}",
                result.tx_hash.unwrap_or_default()
            );
        }
        Err(polymarket_relayer::RelayerError::WalletAlreadyDeployed(addr)) => {
            println!("ℹ️  Safe already deployed at {}", addr);
        }
        Err(e) => return Err(e.into()),
    }

    // ── 4. Set up all approvals in one batch ──
    println!("\nSetting up approvals (USDC + CTF for all exchanges)...");
    let handle = client.setup_approvals().await?;
    println!("Submitted tx: {}", handle.id());

    let result = handle.wait().await?;
    println!(
        "✅ Approvals confirmed! tx: {}",
        result.tx_hash.unwrap_or_default()
    );

    Ok(())
}
