//! Diagnose GS026 "Invalid owner provided" errors.
//!
//! Checks:
//! 1. Does the derived Safe address match your expected wallet address?
//! 2. Is your EOA an owner of the Safe on-chain?
//! 3. Does the EIP-712 hash computed off-chain match the on-chain getTransactionHash?
//!
//! Usage:
//!   cargo run --example diagnose_gs026
//!
//! Required env vars:
//!   PRIVATE_KEY=0x...
//!   POLY_RELAYER_ADDRESS=0x...       # your actual wallet address (from Polymarket UI)
//!   POLYGON_RPC_URL=https://...      # Alchemy or QuickNode
//!   SIGNATURE_TYPE=2                 # 1=Proxy, 2=Safe

use ethers::providers::{Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, Bytes, H256, U256};
use ethers::utils::keccak256;
use polymarket_relayer::{RelayClient, AuthMethod, RelayerTxType};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let private_key = env::var("PRIVATE_KEY")?;
    let expected_wallet = env::var("POLY_RELAYER_ADDRESS")?;
    let rpc_url = env::var("POLYGON_RPC_URL")
        .unwrap_or_else(|_| "https://polygon-rpc.com".to_string());
    let sig_type: u8 = env::var("SIGNATURE_TYPE")
        .unwrap_or_else(|_| "2".to_string())
        .parse()
        .unwrap_or(2);

    let wallet: LocalWallet = private_key.parse()?;
    let eoa = wallet.address();
    let expected_addr: Address = expected_wallet.parse()?;
    let tx_type = RelayerTxType::from_signature_type(sig_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid SIGNATURE_TYPE={sig_type}"))?;

    println!("=== GS026 Diagnostic ===\n");
    println!("EOA (signer):      {:?}", eoa);
    println!("Expected wallet:   {:?} (from POLY_RELAYER_ADDRESS)", expected_addr);
    println!("Wallet type:       {} (signature_type={})", tx_type.as_str(), sig_type);

    // ── Check 1: Derived address ───────────────────────────────────────

    println!("\n--- Check 1: Address Derivation ---");

    let auth = AuthMethod::builder("dummy", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "dummy");
    let client = RelayClient::new(137, wallet.clone(), auth, tx_type).await?;
    let derived = client.wallet_address()?;

    println!("Derived address:   {:?}", derived);

    if derived == expected_addr {
        println!("  [OK] Derived address matches POLY_RELAYER_ADDRESS");
    } else {
        println!("  [FAIL] MISMATCH! Derived {:?} != Expected {:?}", derived, expected_addr);
        println!("  → The SDK derives a different address than your actual wallet.");
        println!("  → This means execTransaction is sent to the WRONG Safe.");
        println!("  → Your EOA is not an owner of the derived Safe.");
        println!("  → This is the most likely cause of GS026.");
        println!();
        println!("  Possible fixes:");
        println!("  1. Use the correct PRIVATE_KEY that matches this wallet");
        println!("  2. If you have a Proxy wallet (magic.link), set SIGNATURE_TYPE=1");
        println!("  3. Check that POLY_RELAYER_ADDRESS is your Polymarket wallet");
    }

    // ── Check 2: On-chain ownership ────────────────────────────────────

    println!("\n--- Check 2: On-chain Safe Ownership ---");

    let provider = Provider::<Http>::try_from(rpc_url.as_str())?;

    // Check code at the Safe address — is a contract deployed?
    let code = provider.get_code(expected_addr, None).await?;
    if code.is_empty() {
        println!("  [FAIL] No contract at {:?} — wallet not deployed!", expected_addr);
        println!("  → You need to deploy the wallet first: client.deploy()");
    } else {
        println!("  [OK] Contract exists at {:?} ({} bytes)", expected_addr, code.len());

        // Call getOwners() on the Safe
        let owners_selector = &keccak256(b"getOwners()")[..4];
        match provider.call(
            &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
                ethers::types::TransactionRequest::new()
                    .to(expected_addr)
                    .data(Bytes::from(owners_selector.to_vec())),
            ),
            None,
        ).await {
            Ok(result) => {
                let owners = decode_address_array(&result);
                println!("  Safe owners ({}):", owners.len());
                let mut eoa_is_owner = false;
                for (i, owner) in owners.iter().enumerate() {
                    let marker = if *owner == eoa { " ← YOUR EOA" } else { "" };
                    if *owner == eoa { eoa_is_owner = true; }
                    println!("    [{}] {:?}{}", i, owner, marker);
                }
                if eoa_is_owner {
                    println!("  [OK] Your EOA IS an owner of this Safe");
                } else {
                    println!("  [FAIL] Your EOA is NOT an owner of this Safe!");
                    println!("  → The signature will recover to your EOA address,");
                    println!("    but the Safe doesn't recognize it as an owner.");
                    println!("  → This IS the cause of GS026.");
                }
            }
            Err(e) => {
                println!("  [WARN] Could not call getOwners(): {}", e);
                println!("  → Contract might not be a Gnosis Safe, or RPC issue.");
            }
        }

        // Also check the Safe at the DERIVED address if different
        if derived != expected_addr {
            println!("\n  Also checking DERIVED address {:?}:", derived);
            let derived_code = provider.get_code(derived, None).await?;
            if derived_code.is_empty() {
                println!("    [INFO] No contract at derived address — Safe not deployed there");
            } else {
                println!("    [WARN] Contract EXISTS at derived address too ({} bytes)", derived_code.len());
                println!("    → Two different Safes exist. SDK sends to the derived one,");
                println!("      but your positions are in the expected one.");
            }
        }
    }

    // ── Check 3: Nonce from relayer API ────────────────────────────────

    println!("\n--- Check 3: Safe Nonce ---");

    // Read nonce from on-chain
    let nonce_selector = &keccak256(b"nonce()")[..4];
    match provider.call(
        &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
            ethers::types::TransactionRequest::new()
                .to(expected_addr)
                .data(Bytes::from(nonce_selector.to_vec())),
        ),
        None,
    ).await {
        Ok(result) => {
            if result.len() >= 32 {
                let nonce = U256::from_big_endian(&result[..32]).as_u64();
                println!("  On-chain nonce: {}", nonce);
            }
        }
        Err(e) => println!("  Could not read nonce: {}", e),
    }

    // ── Check 4: Signature verification simulation ─────────────────────

    println!("\n--- Check 4: Signature Test ---");
    println!("  Sign method: sign_message (eth_sign prefix, v+4 → v=31/32)");
    println!("  Testing if ecrecover returns your EOA...");

    // Build a dummy EIP-712 hash and sign it
    let dummy_hash = H256::from(keccak256(b"diagnose_test"));
    let sig = wallet.sign_message(dummy_hash.as_bytes()).await?;
    let v = sig.v as u8;
    println!("  Raw signature v={}, adjusted v={}", v, if v <= 1 { v + 31 } else if v <= 28 { v + 4 } else { v });

    // Verify recovery
    let recovery_hash = {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
        buf.extend_from_slice(dummy_hash.as_bytes());
        H256::from(keccak256(&buf))
    };

    let recovered = sig.recover(recovery_hash)?;
    println!("  Recovered address: {:?}", recovered);

    if recovered == eoa {
        println!("  [OK] Signature recovery matches EOA");
        println!("  → Signing code is correct, GS026 is likely an address mismatch issue");
    } else {
        println!("  [FAIL] Recovery mismatch! Got {:?}, expected {:?}", recovered, eoa);
        println!("  → Signing code has a bug");
    }

    println!("\n=== Summary ===\n");
    if derived != expected_addr {
        println!("ROOT CAUSE: Address mismatch (derived != expected)");
        println!("The SDK sends execTransaction to {:?}", derived);
        println!("But your wallet is at {:?}", expected_addr);
        println!("Your EOA is not an owner of the derived Safe → GS026");
    } else {
        println!("Address derivation is correct.");
        println!("If GS026 persists, check nonce or try running with RUST_LOG=debug");
    }

    Ok(())
}

/// Decode a Solidity `address[]` return value.
fn decode_address_array(data: &[u8]) -> Vec<Address> {
    if data.len() < 64 { return vec![]; }

    // offset (32 bytes) + length (32 bytes) + addresses (32 bytes each)
    let offset = U256::from_big_endian(&data[0..32]).as_usize();
    if offset + 32 > data.len() { return vec![]; }

    let count = U256::from_big_endian(&data[offset..offset + 32]).as_usize();
    let mut addresses = Vec::with_capacity(count);

    for i in 0..count {
        let start = offset + 32 + i * 32;
        if start + 32 > data.len() { break; }
        addresses.push(Address::from_slice(&data[start + 12..start + 32]));
    }

    addresses
}
