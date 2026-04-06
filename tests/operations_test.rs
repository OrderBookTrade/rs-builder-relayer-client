use ethers::types::U256;
use polymarket_relayer::contracts;
use polymarket_relayer::operations;
use polymarket_relayer::types::{RelayerTxType, TxState};

// ── Type tests ──

#[test]
fn test_tx_state_terminal() {
    assert!(TxState::Confirmed.is_terminal());
    assert!(TxState::Failed.is_terminal());
    assert!(TxState::Invalid.is_terminal());
    assert!(!TxState::New.is_terminal());
    assert!(!TxState::Executed.is_terminal());
    assert!(!TxState::Mined.is_terminal());
}

#[test]
fn test_tx_state_success() {
    assert!(TxState::Confirmed.is_success());
    assert!(TxState::Mined.is_success());
    assert!(!TxState::Failed.is_success());
    assert!(!TxState::Invalid.is_success());
    assert!(!TxState::New.is_success());
}

#[test]
fn test_relayer_tx_type_str() {
    assert_eq!(RelayerTxType::Safe.as_str(), "SAFE");
    assert_eq!(RelayerTxType::Proxy.as_str(), "PROXY");
}

// ── Contract address sanity ──

#[test]
fn test_contract_addresses_are_valid() {
    use ethers::types::Address;
    let addrs = [
        contracts::USDC_E,
        contracts::CTF,
        contracts::CTF_EXCHANGE,
        contracts::NEG_RISK_EXCHANGE,
        contracts::NEG_RISK_ADAPTER,
        contracts::PROXY_FACTORY,
        contracts::SAFE_FACTORY,
        contracts::SAFE_MULTISEND,
        contracts::RELAY_HUB,
        contracts::ZERO_ADDRESS,
    ];
    for addr_str in addrs {
        let parsed: Result<Address, _> = addr_str.parse();
        assert!(parsed.is_ok(), "failed to parse address: {}", addr_str);
    }
}

#[test]
fn test_parse_address_helper() {
    let addr = contracts::parse_address(contracts::USDC_E).unwrap();
    assert_eq!(format!("{:?}", addr).to_lowercase(), contracts::USDC_E.to_lowercase());
}

#[test]
fn test_parse_address_invalid() {
    let result = contracts::parse_address("not_an_address");
    assert!(result.is_err());
}

// ── Operations: approve ──

#[test]
fn test_approve_usdc_for_ctf_exchange() {
    let tx = operations::approve_usdc_for_ctf_exchange();
    assert_eq!(tx.to, contracts::USDC_E);
    assert_eq!(tx.value, "0");
    assert!(tx.data.starts_with("0x"));
    // ERC20 approve selector: 0x095ea7b3
    assert!(tx.data.starts_with("0x095ea7b3"), "wrong selector: {}", tx.data);
}

#[test]
fn test_approve_usdc_for_neg_risk() {
    let tx = operations::approve_usdc_for_neg_risk_exchange();
    assert_eq!(tx.to, contracts::USDC_E);
    assert!(tx.data.starts_with("0x095ea7b3"));
}

#[test]
fn test_approve_ctf_for_ctf_exchange() {
    let tx = operations::approve_ctf_for_ctf_exchange();
    assert_eq!(tx.to, contracts::CTF);
    // ERC1155 setApprovalForAll selector: 0xa22cb465
    assert!(tx.data.starts_with("0xa22cb465"), "wrong selector: {}", tx.data);
}

#[test]
fn test_approve_ctf_for_neg_risk_adapter() {
    let tx = operations::approve_ctf_for_neg_risk_adapter();
    assert_eq!(tx.to, contracts::CTF);
    assert!(tx.data.starts_with("0xa22cb465"));
}

#[test]
fn test_custom_approve() {
    let tx = operations::approve(
        "0x0000000000000000000000000000000000000001",
        "0x0000000000000000000000000000000000000002",
        U256::from(1_000_000u64),
    );
    assert_eq!(tx.to, "0x0000000000000000000000000000000000000001");
    assert!(tx.data.starts_with("0x095ea7b3"));
    assert_eq!(tx.value, "0");
}

// ── Operations: redeem ──

#[test]
fn test_redeem_regular() {
    let condition_id = [0xAA; 32];
    let tx = operations::redeem_regular(condition_id, &[1, 2]);

    assert_eq!(tx.to, contracts::CTF);
    assert_eq!(tx.value, "0");
    assert!(tx.data.starts_with("0x"));
    // The calldata should be > just the selector (4 bytes = 8 hex chars + "0x")
    assert!(tx.data.len() > 10);
}

#[test]
fn test_redeem_neg_risk() {
    let condition_id = [0xBB; 32];
    let tx = operations::redeem_neg_risk_positions(condition_id, &[1, 2]);

    assert_eq!(tx.to, contracts::NEG_RISK_ADAPTER);
    assert_eq!(tx.value, "0");
    assert!(tx.data.starts_with("0x"));
}

#[test]
fn test_redeem_positions_custom_collateral() {
    let condition_id = [0xCC; 32];
    let parent = [0u8; 32];
    let tx = operations::redeem_positions(contracts::USDC_E, parent, condition_id, &[1, 2]);

    assert_eq!(tx.to, contracts::CTF);
}

// ── Operations: split/merge ──

#[test]
fn test_split_regular() {
    let condition_id = [0xDD; 32];
    let amount = U256::from(5_000_000u64); // 5 USDC
    let tx = operations::split_regular(condition_id, &[1, 2], amount);

    assert_eq!(tx.to, contracts::CTF);
    assert_eq!(tx.value, "0");
    assert!(tx.data.len() > 10);
}

#[test]
fn test_merge_regular() {
    let condition_id = [0xEE; 32];
    let amount = U256::from(5_000_000u64);
    let tx = operations::merge_regular(condition_id, &[1, 2], amount);

    assert_eq!(tx.to, contracts::CTF);
    assert_eq!(tx.value, "0");
    assert!(tx.data.len() > 10);
}

#[test]
fn test_split_and_merge_produce_different_calldata() {
    let condition_id = [0xFF; 32];
    let amount = U256::from(1_000_000u64);
    let split_tx = operations::split_regular(condition_id, &[1, 2], amount);
    let merge_tx = operations::merge_regular(condition_id, &[1, 2], amount);

    // Both target CTF but with different function selectors
    assert_eq!(split_tx.to, merge_tx.to);
    assert_ne!(split_tx.data, merge_tx.data, "split and merge should have different calldata");
}

// ── Operations: set_approval_for_all ──

#[test]
fn test_set_approval_for_all() {
    let tx = operations::set_approval_for_all(
        contracts::CTF,
        contracts::CTF_EXCHANGE,
        true,
    );
    assert_eq!(tx.to, contracts::CTF);
    assert!(tx.data.starts_with("0xa22cb465"));

    // The bool `true` should appear in the encoded args
    // encoded as 32-byte word with last byte = 1
}

#[test]
fn test_set_approval_for_all_revoke() {
    let tx_approve = operations::set_approval_for_all(contracts::CTF, contracts::CTF_EXCHANGE, true);
    let tx_revoke = operations::set_approval_for_all(contracts::CTF, contracts::CTF_EXCHANGE, false);

    // Same selector, same addresses, but different bool encoding
    assert_ne!(tx_approve.data, tx_revoke.data);
}
