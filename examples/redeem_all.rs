//! Scan all positions via the Polymarket Data API, then redeem settled ones.
//!
//! Three execution modes:
//!   - **Sequential** (default): one relay request per condition, wait for confirmation
//!     between each. Safe for any number of positions — avoids nonce collisions.
//!   - **Batch** (`--batch`): pack ALL redeems into a single relay request.
//!     Most gas-efficient, but limited by relay bot gas budget.
//!   - **Direct fallback**: on relayer 429, falls back to on-chain via Safe
//!     (Proxy wallets do not support direct fallback).
//!
//! Usage:
//!   cargo run --example redeem_all                        # dry-run
//!   cargo run --example redeem_all -- --execute           # sequential (default)
//!   cargo run --example redeem_all -- --execute --batch   # batch all in one tx
//!   cargo run --example redeem_all -- --execute --delay 8 # 8s between sequential
//!
//! Required env vars (or .env file):
//!   PRIVATE_KEY=0x...
//!   POLY_RELAYER_ADDRESS=0x...
//!   BUILDER_KEY / BUILDER_SECRET / BUILDER_PASSPHRASE
//!   POLYGON_RPC_URL=https://...          # Alchemy/QuickNode (recommended)
//!   SIGNATURE_TYPE=2                     # 0=EOA, 1=Proxy, 2=Safe (default)

use ethers::signers::LocalWallet;
use ethers::types::Address;
use polymarket_client_sdk::data::Client as DataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_relayer::{
    operations, AuthMethod, DirectExecutor, RelayClient, RelayerTxType, Transaction,
};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::env;
use tokio::time::Duration;

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

    // ── Parse CLI args ─────────────────────────────────────────────────
    let args: Vec<String> = env::args().collect();
    let execute = args.iter().any(|a| a == "--execute");
    let batch_mode = args.iter().any(|a| a == "--batch");
    let delay_secs: u64 = args.iter()
        .position(|a| a == "--delay")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    // ── 1. Build clients ───────────────────────────────────────────────

    let private_key = env::var("PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("Missing PRIVATE_KEY in .env"))?;
    let wallet_address = env::var("POLY_RELAYER_ADDRESS")
        .map_err(|_| anyhow::anyhow!("Missing POLY_RELAYER_ADDRESS in .env"))?;
    let rpc_url = env::var("POLYGON_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC.to_string());

    let auth = if let (Ok(key), Ok(secret), Ok(pass)) = (
        env::var("BUILDER_KEY"),
        env::var("BUILDER_SECRET"),
        env::var("BUILDER_PASSPHRASE"),
    ) {
        println!("Auth:   Builder (HMAC) — gasless mode");
        AuthMethod::builder(&key, &secret, &pass)
    } else if let Ok(api_key) = env::var("POLY_RELAYER_API_KEY") {
        println!("Auth:   Relayer key — gasless mode");
        AuthMethod::relayer_key(&api_key, &wallet_address)
    } else {
        anyhow::bail!(
            "Set BUILDER_KEY/SECRET/PASSPHRASE or POLY_RELAYER_API_KEY in .env"
        );
    };

    let wallet: LocalWallet = private_key.parse()?;

    let sig_type: u8 = env::var("SIGNATURE_TYPE")
        .unwrap_or_else(|_| "2".to_string())
        .parse()
        .unwrap_or(2);
    let tx_type = RelayerTxType::from_signature_type(sig_type)
        .unwrap_or_else(|| {
            eprintln!("Unknown SIGNATURE_TYPE={sig_type}, defaulting to Safe (2)");
            RelayerTxType::Safe
        });

    let mut client = RelayClient::new(137, wallet.clone(), auth, tx_type).await?;
    client.set_rpc_url(rpc_url.clone());

    let direct = match tx_type {
        RelayerTxType::Proxy => {
            let proxy_addr: Address = wallet_address.parse()?;
            DirectExecutor::new_proxy_with_address(&rpc_url, wallet, 137, proxy_addr)?
        }
        _ => DirectExecutor::with_type(&rpc_url, wallet, 137, tx_type)?,
    };
    let matic_balance = direct.get_matic_balance().await.unwrap_or(0.0);

    println!("EOA:    {:?}", client.signer_address());
    println!("Wallet: {:?} ({})", client.wallet_address()?, tx_type.as_str());
    println!("MATIC:  {:.4} (for direct fallback)", matic_balance);
    let mode_str = if batch_mode { "batch".to_string() } else { format!("sequential ({}s delay)", delay_secs) };
    println!("Mode:   {}", mode_str);

    // ── 2. Fetch all positions ─────────────────────────────────────────

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

    // ── 3. Display + filter ────────────────────────────────────────────

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
        let status = if pos.redeemable { if won { "WON" } else { "LOST" } } else { "ACTIVE" };
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
            i + 1, title, pos.outcome, pos.size, status, value_str, action,
        );

        if pos.redeemable {
            if won { expected_usdc += pos.size; }
            redeemable.push(pos);
        } else {
            active_count += 1;
        }
    }

    println!();
    println!(
        "  Redeemable: {} position(s) | Expected USDC: ~${:.2}",
        redeemable.len(), expected_usdc,
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

    // ── 4. Build redeem transactions (deduplicate by condition_id) ──────

    let mut seen_conditions = HashSet::new();
    let mut redeem_txs: Vec<(String, Transaction)> = Vec::new(); // (title, tx)

    for pos in &redeemable {
        let cid = format!("0x{}", hex::encode(pos.condition_id));
        if !seen_conditions.insert(cid.clone()) {
            continue;
        }

        let cid_bytes: [u8; 32] = *pos.condition_id;
        let tx = if pos.negative_risk {
            operations::redeem_neg_risk_positions(cid_bytes, &[1, 2])
        } else {
            operations::redeem_regular(cid_bytes, &[1, 2])
        };
        redeem_txs.push((truncate(&pos.title, 42), tx));
    }

    let total = redeem_txs.len();
    println!("=== EXECUTING {} REDEMPTION(S) [{}] ===\n", total, if batch_mode { "batch" } else { "sequential" });

    // ── 5a. Batch mode: single relay request ───────────────────────────

    if batch_mode {
        let txs: Vec<Transaction> = redeem_txs.iter().map(|(_, tx)| tx.clone()).collect();
        let titles: Vec<&str> = redeem_txs.iter().map(|(t, _)| t.as_str()).collect();

        match client.execute_batch(txs, "Batch redeem all").await {
            Ok(result) => {
                let hash = result.tx_hash.as_deref().unwrap_or("unknown");
                println!("  [OK]   Batch redeemed {} condition(s) | tx: {}", total, short_hash(hash));
                for t in &titles {
                    println!("         - {}", t);
                }
            }
            Err(e) => {
                println!("  [FAIL] Batch failed: {}", e);
                println!("         Try without --batch (sequential mode) instead.");
            }
        }

        println!("\n=== DONE ===");
        println!("Expected USDC recovered: ~${:.2}", expected_usdc);
        return Ok(());
    }

    // ── 5b. Sequential mode: one-at-a-time with wait ───────────────────

    let batches: Vec<Vec<Transaction>> = redeem_txs.iter()
        .map(|(_, tx)| vec![tx.clone()])
        .collect();
    let titles: Vec<String> = redeem_txs.iter().map(|(t, _)| t.clone()).collect();

    let delay = Duration::from_secs(delay_secs);
    let mut success_count = 0u32;
    let mut fail_count = 0u32;

    match client.execute_sequential(
        batches,
        Some(delay),
        Some(&|completed, total| {
            println!("  ... redeemed {}/{} positions", completed, total);
        }),
    ).await {
        Ok(results) => {
            for (i, result) in results.iter().enumerate() {
                let title = titles.get(i).map(|s| s.as_str()).unwrap_or("?");
                let hash = result.tx_hash.as_deref().unwrap_or("unknown");
                println!(
                    "  [OK]   \"{}\" | gasless | tx: {}",
                    title, short_hash(hash),
                );
                success_count += 1;
            }
        }
        Err(e) => {
            // Sequential stops at first error — count what succeeded
            println!("  [FAIL] Sequential stopped: {}", e);
            fail_count += 1;
        }
    }

    // ── 6. Summary ─────────────────────────────────────────────────────

    println!("\n=== DONE ===");
    println!("Redeemed: {success_count}/{total} condition(s)");
    if fail_count > 0 {
        println!("Failed:   {fail_count}");
    }
    println!("Expected USDC recovered: ~${:.2}", expected_usdc);

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
