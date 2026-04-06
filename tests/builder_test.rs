use polymarket_relayer::builder::safe::encode_multisend;
use polymarket_relayer::types::Transaction;

#[test]
fn test_encode_multisend_single_tx() {
    let tx = Transaction {
        to: "0x0000000000000000000000000000000000000001".to_string(),
        data: "0xdeadbeef".to_string(),
        value: "0".to_string(),
    };

    let encoded = encode_multisend(&[tx]).unwrap();
    // Should contain: 1 byte op + 20 bytes addr + 32 bytes value + 32 bytes len + 4 bytes data = 89
    assert_eq!(encoded.len(), 89);
    // First byte should be 0 (Call)
    assert_eq!(encoded[0], 0);
}

#[test]
fn test_encode_multisend_multiple_txs() {
    let tx1 = Transaction {
        to: "0x0000000000000000000000000000000000000001".to_string(),
        data: "0xaa".to_string(),
        value: "0".to_string(),
    };
    let tx2 = Transaction {
        to: "0x0000000000000000000000000000000000000002".to_string(),
        data: "0xbbcc".to_string(),
        value: "0".to_string(),
    };

    let encoded = encode_multisend(&[tx1, tx2]).unwrap();

    // tx1: 1 + 20 + 32 + 32 + 1 = 86
    // tx2: 1 + 20 + 32 + 32 + 2 = 87
    assert_eq!(encoded.len(), 86 + 87);
}

#[test]
fn test_encode_multisend_preserves_data() {
    let tx = Transaction {
        to: "0x0000000000000000000000000000000000000001".to_string(),
        data: "0xaabbccdd".to_string(),
        value: "0".to_string(),
    };

    let encoded = encode_multisend(&[tx]).unwrap();

    // The data bytes should appear at the end
    let data_start = 1 + 20 + 32 + 32; // op + addr + value + len
    assert_eq!(&encoded[data_start..], &[0xaa, 0xbb, 0xcc, 0xdd]);
}

#[test]
fn test_encode_multisend_empty() {
    let encoded = encode_multisend(&[]).unwrap();
    assert!(encoded.is_empty());
}

#[test]
fn test_encode_multisend_invalid_address() {
    let tx = Transaction {
        to: "not_an_address".to_string(),
        data: "0x".to_string(),
        value: "0".to_string(),
    };

    let result = encode_multisend(&[tx]);
    assert!(result.is_err());
}

#[test]
fn test_encode_multisend_data_without_0x_prefix() {
    let tx = Transaction {
        to: "0x0000000000000000000000000000000000000001".to_string(),
        data: "deadbeef".to_string(), // no 0x prefix
        value: "0".to_string(),
    };

    let encoded = encode_multisend(&[tx]);
    assert!(encoded.is_ok(), "should handle data without 0x prefix");
}
