//! Redeem settled positions using a magic.link wallet (Proxy, signature_type=1).
//!
//! This example demonstrates:
//!   - Using RelayerTxType::Proxy for magic.link wallets
//!   - Gasless redemption via the relayer
//!   - Direct fallback via DirectExecutor::new_proxy_with_address()
//!
//! Required env vars (or .env file):
//!   POLYMARKET_PRIVATE_KEY=0x...        # magic.link EOA private key
//!   PROXY_WALLET_ADDRESS=0x...          # your Polymarket proxy wallet address
//!   BUILDER_KEY=...                     # Builder HMAC key
//!   BUILDER_SECRET=...                  # Builder HMAC secret (base64)
//!   BUILDER_PASSPHRASE=...              # Builder HMAC passphrase
//!   POLYGON_RPC_URL=https://...         # Alchemy or QuickNode (NOT polygon-rpc.com)
//!
//! Usage:
//!   cargo run --example redeem_magic                 # dry-run
//!   cargo run --example redeem_magic -- --execute    # actually redeem

use ethers::signers::LocalWallet;
use ethers::types::Address;
use polymarket_client_sdk::data::Client as DataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_relayer::{
    operations, AuthMethod, DirectExecutor, RelayClient, RelayerError, RelayerTxType, Transaction,
};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,polymarket_relayer=debug".into()),
        )
        .init();

    let _ = dotenvy::dotenv();

    let execute = env::args().any(|a| a == "--execute");

    // ── Load credentials ───────────────────────────────────────────────

    let private_key = env::var("POLYMARKET_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("Missing POLYMARKET_PRIVATE_KEY"))?;
    let proxy_wallet_str = env::var("PROXY_WALLET_ADDRESS")
        .map_err(|_| anyhow::anyhow!("Missing PROXY_WALLET_ADDRESS"))?;
    let rpc_url = env::var("POLYGON_RPC_URL")
        .map_err(|_| anyhow::anyhow!("Missing POLYGON_RPC_URL — use Alchemy or QuickNode, not polygon-rpc.com"))?;

    let wallet: LocalWallet = private_key.parse()?;
    let proxy_address: Address = proxy_wallet_str.parse()?;

    // ── Build relayer client (gasless, Proxy mode) ─────────────────────

    let auth = AuthMethod::builder(
        &env::var("BUILDER_KEY").map_err(|_| anyhow::anyhow!("Missing BUILDER_KEY"))?,
        &env::var("BUILDER_SECRET").map_err(|_| anyhow::anyhow!("Missing BUILDER_SECRET"))?,
        &env::var("BUILDER_PASSPHRASE").map_err(|_| anyhow::anyhow!("Missing BUILDER_PASSPHRASE"))?,
    );

    let mut client = RelayClient::new(137, wallet.clone(), auth, RelayerTxType::Proxy).await?;
    client.set_rpc_url(rpc_url.clone());

    // Direct executor (proxy mode) for fallback
    let direct = DirectExecutor::new_proxy_with_address(&rpc_url, wallet, 137, proxy_address)?;
    let matic_balance = direct.get_matic_balance().await.unwrap_or(0.0);

    println!("=== Magic.link Proxy Wallet Redeemer ===\n");
    println!("EOA:          {:?}", client.signer_address());
    println!("Proxy wallet: {:?}", proxy_address);
    println!("Derived:      {:?}", client.wallet_address()?);
    println!("MATIC:        {:.4} (for direct fallback)", matic_balance);
    println!("Mode:         {}", if execute { "EXECUTE" } else { "DRY RUN" });

    // ── Fetch positions ────────────────────────────────────────────────

    let data = DataClient::default();
    let wallet_addr: polymarket_client_sdk::types::Address = proxy_wallet_str
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid PROXY_WALLET_ADDRESS: {e}"))?;

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
        println!("\nNo positions found for this wallet.");
        return Ok(());
    }

    // ── Filter redeemable ──────────────────────────────────────────────

    println!("\n=== POSITIONS ===\n");
    println!(
        "  {:<3} {:<46} {:<6} {:<10} {:<9} {}",
        "#", "Market", "Side", "Shares", "Status", "Action"
    );

    let mut redeemable = Vec::new();
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

        println!(
            "  {:<3} {:<46} {:<6} {:<10} {:<9} -> {}",
            i + 1, title, pos.outcome, pos.size, status, action,
        );

        if pos.redeemable {
            if won { expected_usdc += pos.size; }
            redeemable.push(pos);
        }
    }

    println!("\n  Redeemable: {} | Expected USDC: ~${:.2}\n", redeemable.len(), expected_usdc);

    if redeemable.is_empty() {
        println!("Nothing to redeem.");
        return Ok(());
    }

    if !execute {
        println!("=== DRY RUN — run with --execute to send transactions ===");
        return Ok(());
    }

    // ── Execute redemptions ────────────────────────────────────────────

    println!("=== EXECUTING REDEMPTIONS ===\n");

    let mut redeemed_ids = HashSet::new();
    let mut success_count = 0u32;
    let mut fail_count = 0u32;

    for pos in &redeemable {
        let cid = format!("0x{}", hex::encode(pos.condition_id));
        if redeemed_ids.contains(&cid) { continue; }
        redeemed_ids.insert(cid.clone());

        let title = truncate(&pos.title, 42);
        let cid_bytes: [u8; 32] = *pos.condition_id;

        let tx = if pos.negative_risk {
            operations::redeem_neg_risk_positions(cid_bytes, &[1, 2])
        } else {
            operations::redeem_regular(cid_bytes, &[1, 2])
        };

        // Try gasless first
        match try_relayer(&client, &tx, &pos.title).await {
            Ok(hash) => {
                println!("  [OK]   \"{}\" | gasless | tx: {}", title, short_hash(&hash));
                success_count += 1;
                continue;
            }
            Err(RelayerError::QuotaExhausted) => {
                println!("  [429]  \"{}\" | quota hit — direct fallback", title);
            }
            Err(e) => {
                println!("  [WARN] \"{}\" | relayer error: {} — trying direct", title, e);
            }
        }

        // Fallback: direct proxy execution
        match direct.execute(&tx).await {
            Ok(r) if r.success => {
                println!(
                    "  [OK]   \"{}\" | direct | gas: {:.5} MATIC | tx: {}",
                    title, r.gas_cost_matic, short_hash(&r.tx_hash),
                );
                success_count += 1;
            }
            Ok(r) => {
                println!("  [FAIL] \"{}\" | reverted | tx: {}", title, short_hash(&r.tx_hash));
                fail_count += 1;
            }
            Err(e) => {
                println!("  [FAIL] \"{}\" | {}", title, e);
                fail_count += 1;
            }
        }
    }

    println!("\n=== DONE ===");
    println!("Redeemed: {success_count}/{} condition(s)", redeemed_ids.len());
    if fail_count > 0 { println!("Failed: {fail_count}"); }
    println!("Expected USDC: ~${:.2}", expected_usdc);

    Ok(())
}

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
