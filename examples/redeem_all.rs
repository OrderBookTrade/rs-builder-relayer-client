//! Scan all positions via the Polymarket Data API, then redeem settled ones.
//!
//! Two modes (auto-detected, matches the Python redeem service):
//!   - GASLESS via Polymarket relayer (needs BUILDER_KEY, no MATIC needed)
//!   - DIRECT on-chain via GnosisSafe  (fallback on 429, needs MATIC for gas)
//!
//! Usage:
//!   cargo run --example redeem_all                 # dry-run (default)
//!   cargo run --example redeem_all -- --execute    # actually redeem
//!
//! Required env vars (or .env file):
//!   PRIVATE_KEY=0x...
//!   POLY_RELAYER_ADDRESS=0x...          # your Safe wallet address
//!   BUILDER_KEY=...                     # Builder HMAC key
//!   BUILDER_SECRET=...                  # Builder HMAC secret (base64)
//!   BUILDER_PASSPHRASE=...              # Builder HMAC passphrase
//!   POLYGON_RPC_URL=https://...         # (optional, for direct fallback)

use ethers::signers::LocalWallet;
use polymarket_client_sdk::data::Client as DataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_relayer::{
    operations, AuthMethod, DirectExecutor, RelayClient, RelayerError, RelayerTxType, Transaction,
};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::env;

const DEFAULT_RPC: &str = "https://polygon-rpc.com";

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

    let private_key = env::var("PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("Missing PRIVATE_KEY in .env"))?;
    let wallet_address = env::var("POLY_RELAYER_ADDRESS")
        .map_err(|_| anyhow::anyhow!("Missing POLY_RELAYER_ADDRESS in .env"))?;
    let rpc_url = env::var("POLYGON_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC.to_string());

    // ── 1. Build clients ────────────────────────────────────────────────
    // Relayer client (gasless) + direct executor (fallback, needs MATIC)

    let auth = if let (Ok(key), Ok(secret), Ok(pass)) = (
        env::var("BUILDER_KEY"),
        env::var("BUILDER_SECRET"),
        env::var("BUILDER_PASSPHRASE"),
    ) {
        println!("Auth:   Builder (HMAC) — gasless mode, direct fallback ready");
        AuthMethod::builder(&key, &secret, &pass)
    } else if let Ok(api_key) = env::var("POLY_RELAYER_API_KEY") {
        println!("Auth:   Relayer key — gasless mode, direct fallback ready");
        AuthMethod::relayer_key(&api_key, &wallet_address)
    } else {
        anyhow::bail!(
            "Set BUILDER_KEY/SECRET/PASSPHRASE or POLY_RELAYER_API_KEY in .env"
        );
    };

    let wallet: LocalWallet = private_key.parse()?;
    let client = RelayClient::new(137, wallet.clone(), auth, RelayerTxType::Safe).await?;

    // Direct executor for on-chain fallback when relayer returns 429
    let direct = DirectExecutor::new(&rpc_url, wallet, 137)?;
    let matic_balance = direct.get_matic_balance().await.unwrap_or(0.0);

    println!("EOA:    {:?}", client.signer_address());
    println!("Safe:   {:?}", client.wallet_address()?);
    println!("MATIC:  {:.4} (for direct fallback)", matic_balance);

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

    // ── 4. Redeem: try relayer first, fallback to direct on 429 ─────────

    println!("=== EXECUTING REDEMPTIONS ===\n");

    let mut redeemed_ids = HashSet::new();
    let mut success_count = 0u32;
    let mut fail_count = 0u32;
    let mut total_gas_matic = 0.0f64;

    for pos in &redeemable {
        let cid = format!("0x{}", hex::encode(pos.condition_id));

        if redeemed_ids.contains(&cid) {
            continue;
        }
        redeemed_ids.insert(cid.clone());

        let title = truncate(&pos.title, 42);
        let won = pos.cur_price >= Decimal::new(95, 2);
        let contract_label = if pos.negative_risk { "NegRisk" } else { "CTF" };

        let cid_bytes: [u8; 32] = *pos.condition_id;
        let tx = if pos.negative_risk {
            operations::redeem_neg_risk_positions(cid_bytes, &[1, 2])
        } else {
            operations::redeem_regular(cid_bytes, &[1, 2])
        };

        let usdc_label = if won {
            format!("+${:.2}", pos.size)
        } else {
            "$0.00".to_string()
        };

        // Try gasless relayer first
        match try_relayer(&client, &tx, &pos.title).await {
            Ok(tx_hash) => {
                println!(
                    "  [OK]   \"{}\" | {} | {} | gasless | tx: {}",
                    title, usdc_label, contract_label, short_hash(&tx_hash),
                );
                success_count += 1;
                continue;
            }
            Err(RelayerError::QuotaExhausted) => {
                println!(
                    "  [429]  \"{}\" | Relayer quota hit — falling back to direct",
                    title
                );
            }
            Err(e) => {
                println!(
                    "  [WARN] \"{}\" | Relayer error: {} — trying direct",
                    title, e
                );
            }
        }

        // Fallback: direct on-chain via Safe
        match direct.execute(&tx).await {
            Ok(result) if result.success => {
                total_gas_matic += result.gas_cost_matic;
                println!(
                    "  [OK]   \"{}\" | {} | {} | direct | gas: {:.5} MATIC | tx: {}",
                    title,
                    usdc_label,
                    contract_label,
                    result.gas_cost_matic,
                    short_hash(&result.tx_hash),
                );
                success_count += 1;
            }
            Ok(result) => {
                println!(
                    "  [FAIL] \"{}\" | reverted | tx: {}",
                    title,
                    short_hash(&result.tx_hash)
                );
                fail_count += 1;
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
    if total_gas_matic > 0.0 {
        println!("Gas spent (direct):     ~{:.6} MATIC", total_gas_matic);
    }

    Ok(())
}

/// Try to execute via the gasless relayer. Returns tx hash on success.
async fn try_relayer(
    client: &RelayClient,
    tx: &Transaction,
    description: &str,
) -> polymarket_relayer::Result<String> {
    let handle = client
        .execute(vec![tx.clone()], &format!("Redeem: {}", description))
        .await?;
    let tx_id = handle.id().to_string();
    let result = handle.wait().await?;
    Ok(result.tx_hash.unwrap_or(tx_id))
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
