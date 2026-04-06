use ethers::signers::{LocalWallet, Signer};
use ethers::types::Address;
use polymarket_relayer::builder::derive::{derive_proxy_address, derive_safe_address};
use polymarket_relayer::{AuthMethod, RelayClient, RelayerTxType};

// ── Address derivation ──

#[test]
fn test_derive_safe_address_deterministic() {
    let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
        .parse()
        .unwrap();
    let addr1 = derive_safe_address(signer).unwrap();
    let addr2 = derive_safe_address(signer).unwrap();
    assert_eq!(addr1, addr2, "derivation should be deterministic");
}

#[test]
fn test_derive_proxy_address_deterministic() {
    let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
        .parse()
        .unwrap();
    let addr1 = derive_proxy_address(signer).unwrap();
    let addr2 = derive_proxy_address(signer).unwrap();
    assert_eq!(addr1, addr2);
}

#[test]
fn test_safe_and_proxy_derive_different_addresses() {
    let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
        .parse()
        .unwrap();
    let safe = derive_safe_address(signer).unwrap();
    let proxy = derive_proxy_address(signer).unwrap();
    assert_ne!(safe, proxy, "Safe and Proxy should derive different addresses");
}

#[test]
fn test_different_signers_derive_different_addresses() {
    let signer1: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
        .parse()
        .unwrap();
    let signer2: Address = "0x0000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let safe1 = derive_safe_address(signer1).unwrap();
    let safe2 = derive_safe_address(signer2).unwrap();
    assert_ne!(safe1, safe2);
}

// ── Known Safe address vector (disabled until derive formula verified on-chain)
// Uncomment once confirmed against a real deployment:
// #[test]
// fn test_derive_safe_address_known_vector() {
//     let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5".parse().unwrap();
//     let expected: Address = "0x...".parse().unwrap();  // confirm via relayer API
//     let derived = derive_safe_address(signer).unwrap();
//     assert_eq!(derived, expected);
// }

// ── Client instantiation ──

#[tokio::test]
async fn test_client_new_safe() {
    let wallet: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let auth = AuthMethod::relayer_key("test-key", "0xtest");
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe).await;
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_client_new_proxy() {
    let wallet: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let auth = AuthMethod::relayer_key("test-key", "0xtest");
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Proxy).await;
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_client_signer_address() {
    let wallet: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let expected = wallet.address();
    let auth = AuthMethod::relayer_key("test-key", "0xtest");
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe)
        .await
        .unwrap();
    assert_eq!(client.signer_address(), expected);
}

#[tokio::test]
async fn test_client_wallet_address_safe() {
    let wallet: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let auth = AuthMethod::relayer_key("test-key", "0xtest");
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe)
        .await
        .unwrap();

    let derived = client.wallet_address();
    assert!(derived.is_ok());
    // Should not equal the signer address
    assert_ne!(
        client.signer_address(),
        derived.unwrap(),
        "Safe address should differ from signer"
    );
}

#[tokio::test]
async fn test_client_execute_empty_txs_errors() {
    let wallet: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let auth = AuthMethod::relayer_key("test-key", "0xtest");
    let client = RelayClient::new(137, wallet, auth, RelayerTxType::Safe)
        .await
        .unwrap();

    let result = client.execute(vec![], "empty").await;
    assert!(result.is_err(), "should reject empty transaction list");
}
