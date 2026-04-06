//! Scan all positions via the Polymarket Data API, then gaslessly redeem
//! every settled position through the Builder Relayer — no gas needed.
//!
//! This is the killer use-case: one command to claim all your winnings.
//!
//! Usage:
//!   cargo run --example redeem_all                 # dry-run (default)
//!   cargo run --example redeem_all -- --execute    # actually redeem
//!
//! Required env vars (or .env file):
//!   PRIVATE_KEY=0x...
//!   POLY_RELAYER_API_KEY=...
//!   POLY_RELAYER_ADDRESS=0x...     # your Safe/proxy wallet address

use ethers::signers::LocalWallet;
use polymarket_client_sdk::data::Client as DataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_relayer::{operations, AuthMethod, RelayClient, RelayerTxType};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let _ = dotenvy::dotenv();

    let execute = env::args().any(|a| a == "--execute");

    let private_key = env::var("PRIVATE_KEY")?;
    let api_key = env::var("POLY_RELAYER_API_KEY")?;
    let wallet_address = env::var("POLY_RELAYER_ADDRESS")?;

    // ── 1. Build the gasless relayer client ──────────────────────────────

    let wallet: LocalWallet = private_key.parse()?;
    let auth = AuthMethod::relayer_key(&api_key, &wallet_address);
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await?;

    println!("EOA:    {:?}", client.signer_address());
    println!("Safe:   {:?}", client.wallet_address()?);

    // ── 2. Fetch all positions via Polymarket Data API ──────────────────

    let data = DataClient::default();

    let wallet_addr: polymarket_client_sdk::types::Address = wallet_address
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid POLY_RELAYER_ADDRESS: {e}"))?;

    let positions = data
        .positions(
            &PositionsRequest::builder()
                .user(wallet_addr)
                .limit(500)?
                .build(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch positions: {e}"))?;

    if positions.is_empty() {
        println!("\nNo positions found.");
        return Ok(());
    }

    // ── 3. Display position table ───────────────────────────────────────

    println!("\n=== POSITIONS ===\n");
    println!(
        "  {:<3} {:<46} {:<6} {:<10} {:<9} {:<10} {}",
        "#", "Market", "Side", "Shares", "Status", "Value", "Action"
    );

    let mut redeemable = Vec::new();
    let mut active_count = 0u32;
    let mut expected_usdc = Decimal::ZERO;

    for (i, pos) in positions.iter().enumerate() {
        let title = truncate(&pos.title, 44);
        let won = pos.cur_price >= Decimal::new(95, 2);

        let status = if pos.redeemable {
            if won { "WON" } else { "LOST" }
        } else {
            "ACTIVE"
        };

        let action = if pos.redeemable { "REDEEM" } else { "SKIP" };

        let value_str = if pos.redeemable && won {
            format!("${:.2}", pos.size)
        } else if pos.redeemable {
            "$0.00".to_string()
        } else {
            format!("~${:.2}", pos.current_value)
        };

        println!(
            "  {:<3} {:<46} {:<6} {:<10} {:<9} {:<10} -> {}",
            i + 1,
            title,
            pos.outcome,
            pos.size,
            status,
            value_str,
            action,
        );

        if pos.redeemable {
            if won {
                expected_usdc += pos.size;
            }
            redeemable.push(pos);
        } else {
            active_count += 1;
        }
    }

    println!();
    println!(
        "  Redeemable: {} position(s) | Expected USDC: ~${:.2}",
        redeemable.len(),
        expected_usdc,
    );
    if active_count > 0 {
        println!("  Active (skipped): {active_count}");
    }
    println!();

    if redeemable.is_empty() {
        println!("Nothing to redeem.");
        return Ok(());
    }

    if !execute {
        println!("=== DRY RUN (default) — run with --execute to send transactions ===");
        return Ok(());
    }

    // ── 4. Redeem each settled condition via the gasless relayer ─────────
    //
    // Deduplicate by condition_id (both YES and NO share the same one).
    // Each redeem is a separate relayer call (the relayer pays gas).

    println!("=== EXECUTING GASLESS REDEMPTIONS ===\n");

    let mut redeemed_ids = HashSet::new();
    let mut success_count = 0u32;
    let mut fail_count = 0u32;

    for pos in &redeemable {
        let cid = format!("0x{}", hex::encode(pos.condition_id));

        if redeemed_ids.contains(&cid) {
            continue;
        }
        redeemed_ids.insert(cid.clone());

        let title = truncate(&pos.title, 42);
        let won = pos.cur_price >= Decimal::new(95, 2);
        let contract_label = if pos.negative_risk { "NegRisk" } else { "CTF" };

        // Build the right redeem transaction
        // condition_id is FixedBytes<32> from alloy — deref to [u8; 32]
        let cid_bytes: [u8; 32] = *pos.condition_id;
        let tx = if pos.negative_risk {
            operations::redeem_neg_risk_positions(cid_bytes, &[1, 2])
        } else {
            operations::redeem_regular(cid_bytes, &[1, 2])
        };

        // Submit through the gasless relayer
        match client.execute(vec![tx], &format!("Redeem: {}", pos.title)).await {
            Ok(handle) => {
                let tx_id = handle.id().to_string();
                match handle.wait().await {
                    Ok(result) => {
                        let usdc_label = if won {
                            format!("+${:.2}", pos.size)
                        } else {
                            "$0.00".to_string()
                        };
                        println!(
                            "  [OK]   \"{}\" | {} | {} | tx: {}",
                            title,
                            usdc_label,
                            contract_label,
                            short_hash(&result.tx_hash.unwrap_or(tx_id)),
                        );
                        success_count += 1;
                    }
                    Err(e) => {
                        println!("  [FAIL] \"{}\" | {}", title, e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                println!("  [FAIL] \"{}\" | {}", title, e);
                fail_count += 1;
            }
        }
    }

    // ── 5. Summary ──────────────────────────────────────────────────────

    println!("\n=== DONE ===");
    println!(
        "Redeemed: {success_count}/{} condition(s)",
        redeemed_ids.len()
    );
    if fail_count > 0 {
        println!("Failed:   {fail_count}");
    }
    println!("Expected USDC recovered: ~${:.2}", expected_usdc);
    println!("Gas cost: $0.00 (gasless!)");

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 3).collect();
        format!("{cut}...")
    }
}

fn short_hash(h: &str) -> String {
    if h.len() > 14 {
        format!("{}...{}", &h[..8], &h[h.len() - 4..])
    } else {
        h.to_string()
    }
}
