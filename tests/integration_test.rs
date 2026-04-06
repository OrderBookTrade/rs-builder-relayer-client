use ethers::types::{Address, U256};
use polymarket_relayer::builder::derive::{derive_proxy_address, derive_safe_address};
use polymarket_relayer::operations;
use polymarket_relayer::types::Transaction;

#[test]
fn test_derive_safe_address_matches_reference() {
    // Test vector from py-builder-relayer-client
    let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
        .parse()
        .unwrap();
    let expected: Address = "0x6d8c4e9aDF5748Af82Dabe2C6225207770d6B4fa"
        .parse()
        .unwrap();
    assert_eq!(derive_safe_address(signer).unwrap(), expected);
}

#[test]
fn test_derive_proxy_address_deterministic() {
    let signer: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    let addr1 = derive_proxy_address(signer).unwrap();
    let addr2 = derive_proxy_address(signer).unwrap();
    assert_eq!(addr1, addr2);
    assert_ne!(addr1, Address::zero());
}

#[test]
fn test_approve_usdc_for_ctf() {
    let tx = operations::approve_usdc_for_ctf_exchange();
    assert_eq!(tx.to, polymarket_relayer::contracts::USDC_E);
    assert!(tx.data.starts_with("0x"));
    assert_eq!(tx.value, "0");
    // Selector should be approve(address,uint256) = 0x095ea7b3
    assert!(tx.data.starts_with("0x095ea7b3"));
}

#[test]
fn test_set_approval_for_all_ctf() {
    let tx = operations::approve_ctf_for_ctf_exchange();
    assert_eq!(tx.to, polymarket_relayer::contracts::CTF);
    // Selector should be setApprovalForAll(address,bool) = 0xa22cb465
    assert!(tx.data.starts_with("0xa22cb465"));
}

#[test]
fn test_redeem_positions() {
    let condition_id = [0xab; 32];
    let tx = operations::redeem_regular(condition_id, &[1, 2]);
    assert_eq!(tx.to, polymarket_relayer::contracts::CTF);
    assert!(tx.data.starts_with("0x"));
    assert_eq!(tx.value, "0");
}

#[test]
fn test_redeem_neg_risk() {
    let condition_id = [0xcd; 32];
    let tx = operations::redeem_neg_risk_positions(condition_id, &[1, 2]);
    assert_eq!(tx.to, polymarket_relayer::contracts::NEG_RISK_ADAPTER);
}

#[test]
fn test_split_position() {
    let condition_id = [0xef; 32];
    let amount = U256::from(1_000_000u64); // 1 USDC
    let tx = operations::split_regular(condition_id, &[1, 2], amount);
    assert_eq!(tx.to, polymarket_relayer::contracts::CTF);
    assert!(tx.data.starts_with("0x"));
}

#[test]
fn test_merge_position() {
    let condition_id = [0xef; 32];
    let amount = U256::from(1_000_000u64);
    let tx = operations::merge_regular(condition_id, &[1, 2], amount);
    assert_eq!(tx.to, polymarket_relayer::contracts::CTF);
    assert!(tx.data.starts_with("0x"));
}

#[test]
fn test_batch_transactions() {
    let approve_tx = operations::approve_usdc_for_ctf_exchange();
    let condition_id = [0xab; 32];
    let redeem_tx = operations::redeem_regular(condition_id, &[1, 2]);

    let batch = vec![approve_tx, redeem_tx];
    assert_eq!(batch.len(), 2);
}

#[test]
fn test_custom_approve() {
    let usdc = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
    let spender = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
    let tx = operations::approve(usdc, spender, U256::MAX);
    assert_eq!(tx.to, usdc);
    assert!(tx.data.starts_with("0x095ea7b3"));
}

#[tokio::test]
async fn test_safe_multisend_encoding() {
    use polymarket_relayer::builder::safe::encode_multisend;

    let txs = vec![
        Transaction {
            to: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string(),
            data: "0x095ea7b3".to_string(),
            value: "0".to_string(),
        },
        Transaction {
            to: "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_string(),
            data: "0xa22cb465".to_string(),
            value: "0".to_string(),
        },
    ];

    let encoded = encode_multisend(&txs).unwrap();
    assert!(!encoded.is_empty());
}
