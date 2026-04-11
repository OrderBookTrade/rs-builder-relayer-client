//! Precise GS026 nonce & domain separator diagnostic.
//!
//! Compares:
//! 1. Relayer API nonce vs on-chain Safe nonce
//! 2. SDK domain separator vs on-chain domainSeparator()
//! 3. SDK getTransactionHash vs on-chain getTransactionHash()
//!
//! Usage:
//!   cargo run --example diagnose_nonce

use ethers::abi::{encode, Token};
use ethers::providers::{Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, Bytes, H256, U256};
use ethers::utils::keccak256;
use polymarket_relayer::{AuthMethod, RelayClient, RelayerTxType};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let private_key = env::var("PRIVATE_KEY")?;
    let wallet_address_str = env::var("POLY_RELAYER_ADDRESS")?;
    let rpc_url = env::var("POLYGON_RPC_URL")
        .unwrap_or_else(|_| "https://polygon-rpc.com".to_string());

    let wallet: LocalWallet = private_key.parse()?;
    let safe_address: Address = wallet_address_str.parse()?;
    let provider = Provider::<Http>::try_from(rpc_url.as_str())?;

    println!("=== Nonce & Domain Separator Diagnostic ===\n");
    println!("EOA:  {:?}", wallet.address());
    println!("Safe: {:?}\n", safe_address);

    // ── 1. Compare nonces ──────────────────────────────────────────────

    println!("--- Nonce Comparison ---");

    // On-chain nonce
    let nonce_selector = &keccak256(b"nonce()")[..4];
    let onchain_nonce = {
        let result = provider.call(
            &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
                ethers::types::TransactionRequest::new()
                    .to(safe_address)
                    .data(Bytes::from(nonce_selector.to_vec())),
            ),
            None,
        ).await?;
        U256::from_big_endian(&result[..32]).as_u64()
    };
    println!("On-chain Safe nonce:  {}", onchain_nonce);

    // Relayer API nonce
    if let (Ok(key), Ok(secret), Ok(pass)) = (
        env::var("BUILDER_KEY"),
        env::var("BUILDER_SECRET"),
        env::var("BUILDER_PASSPHRASE"),
    ) {
        let auth = AuthMethod::builder(&key, &secret, &pass);
        let client = RelayClient::new(137, wallet.clone(), auth, RelayerTxType::Safe).await?;
        let relayer_nonce = client.get_nonce().await?;
        println!("Relayer API nonce:    {}", relayer_nonce);

        if relayer_nonce == onchain_nonce {
            println!("[OK] Nonces match");
        } else {
            println!("[FAIL] NONCE MISMATCH! relayer={} vs onchain={}", relayer_nonce, onchain_nonce);
            println!("  → SDK signs with nonce={} but Safe verifies with nonce={}", relayer_nonce, onchain_nonce);
            println!("  → EIP-712 hash will differ → ecrecover returns garbage → GS026");
            println!("  → FIX: Read nonce from on-chain instead of relayer API");
        }
    } else {
        println!("[SKIP] No BUILDER_KEY — cannot check relayer API nonce");
    }

    // ── 2. Compare domain separators ───────────────────────────────────

    println!("\n--- Domain Separator Comparison ---");

    // SDK computation
    let domain_type_hash = H256::from(keccak256(
        b"EIP712Domain(uint256 chainId,address verifyingContract)",
    ));
    let sdk_domain_sep = H256::from(keccak256(encode(&[
        Token::FixedBytes(domain_type_hash.as_bytes().to_vec()),
        Token::Uint(U256::from(137u64)),
        Token::Address(safe_address),
    ])));
    println!("SDK domain separator:      {:?}", sdk_domain_sep);

    // On-chain domainSeparator()
    let ds_selector = &keccak256(b"domainSeparator()")[..4];
    match provider.call(
        &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
            ethers::types::TransactionRequest::new()
                .to(safe_address)
                .data(Bytes::from(ds_selector.to_vec())),
        ),
        None,
    ).await {
        Ok(result) if result.len() >= 32 => {
            let onchain_ds = H256::from_slice(&result[..32]);
            println!("On-chain domain separator: {:?}", onchain_ds);

            if sdk_domain_sep == onchain_ds {
                println!("[OK] Domain separators match");
            } else {
                println!("[FAIL] DOMAIN SEPARATOR MISMATCH!");
                println!("  → Safe contract uses a different domain separator");
                println!("  → This means the Safe is a different version");
                println!("  → Possibly an older Safe without chainId in the domain");

                // Try without chainId (older Safe versions)
                let old_type_hash = H256::from(keccak256(
                    b"EIP712Domain(address verifyingContract)",
                ));
                let old_ds = H256::from(keccak256(encode(&[
                    Token::FixedBytes(old_type_hash.as_bytes().to_vec()),
                    Token::Address(safe_address),
                ])));
                if old_ds == onchain_ds {
                    println!("  → CONFIRMED: Old Safe version (no chainId)");
                    println!("  → FIX: Use EIP712Domain(address verifyingContract) only");
                } else {
                    println!("  → Not the old version either. Check Safe implementation.");
                }
            }
        }
        Ok(result) => {
            println!("[WARN] domainSeparator() returned {} bytes", result.len());
        }
        Err(e) => {
            println!("[WARN] domainSeparator() call failed: {}", e);
            println!("  → Safe might not expose this function");
        }
    }

    // ── 3. Compare getTransactionHash ──────────────────────────────────

    println!("\n--- Transaction Hash Comparison ---");

    // Build a dummy tx hash to compare: redeemPositions with condition_id=0x00...01
    let dummy_condition = [0u8; 32];
    let inner_data = build_dummy_redeem_calldata(&dummy_condition);
    let ctf: Address = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".parse()?;

    // SDK computation
    let data_hash = H256::from(keccak256(&inner_data));
    let safe_tx_type_hash = H256::from(keccak256(
        b"SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)",
    ));
    let struct_hash = H256::from(keccak256(encode(&[
        Token::FixedBytes(safe_tx_type_hash.as_bytes().to_vec()),
        Token::Address(ctf),
        Token::Uint(U256::zero()),
        Token::FixedBytes(data_hash.as_bytes().to_vec()),
        Token::Uint(U256::zero()), // operation=Call
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
        Token::Address(Address::zero()),
        Token::Address(Address::zero()),
        Token::Uint(U256::from(onchain_nonce)),
    ])));

    let mut eip712_buf = Vec::with_capacity(66);
    eip712_buf.extend_from_slice(&[0x19, 0x01]);
    eip712_buf.extend_from_slice(sdk_domain_sep.as_bytes());
    eip712_buf.extend_from_slice(struct_hash.as_bytes());
    let sdk_tx_hash = H256::from(keccak256(&eip712_buf));
    println!("SDK tx hash (nonce={}): {:?}", onchain_nonce, sdk_tx_hash);

    // On-chain getTransactionHash
    let gth_selector = &keccak256(
        b"getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)",
    )[..4];
    let encoded_args = encode(&[
        Token::Address(ctf),
        Token::Uint(U256::zero()),
        Token::Bytes(inner_data.clone()),
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
        Token::Uint(U256::zero()),
        Token::Address(Address::zero()),
        Token::Address(Address::zero()),
        Token::Uint(U256::from(onchain_nonce)),
    ]);
    let mut calldata = gth_selector.to_vec();
    calldata.extend_from_slice(&encoded_args);

    match provider.call(
        &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
            ethers::types::TransactionRequest::new()
                .to(safe_address)
                .data(Bytes::from(calldata)),
        ),
        None,
    ).await {
        Ok(result) if result.len() >= 32 => {
            let onchain_tx_hash = H256::from_slice(&result[..32]);
            println!("On-chain tx hash:          {:?}", onchain_tx_hash);

            if sdk_tx_hash == onchain_tx_hash {
                println!("[OK] Transaction hashes match — EIP-712 computation is correct");
            } else {
                println!("[FAIL] TRANSACTION HASH MISMATCH!");
                println!("  → SDK and Safe compute different EIP-712 hashes");
                println!("  → This is the root cause of GS026");
                println!("  → Check domain separator and struct hash encoding");
            }
        }
        Err(e) => {
            println!("[WARN] getTransactionHash() failed: {}", e);
        }
        _ => {}
    }

    Ok(())
}

fn build_dummy_redeem_calldata(condition_id: &[u8; 32]) -> Vec<u8> {
    // redeemPositions(address,bytes32,bytes32,uint256[])
    let selector: [u8; 4] = [0x01, 0xb7, 0x03, 0x7c];
    let encoded = encode(&[
        Token::Address("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap()),
        Token::FixedBytes(vec![0u8; 32]), // parentCollectionId
        Token::FixedBytes(condition_id.to_vec()),
        Token::Array(vec![Token::Uint(U256::from(1u64)), Token::Uint(U256::from(2u64))]),
    ]);
    let mut data = selector.to_vec();
    data.extend_from_slice(&encoded);
    data
}
