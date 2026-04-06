use ethers::abi::encode;
use ethers::abi::Token;
use ethers::types::{Address, U256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::types::Transaction;

/// Build a splitPosition(address,bytes32,bytes32,uint256[],uint256) transaction.
///
/// Splits collateral into outcome tokens.
pub fn split_position(
    collateral_token: &str,
    parent_collection: [u8; 32],
    condition_id: [u8; 32],
    partition: &[u64],
    amount: U256,
) -> Transaction {
    let collateral: Address = collateral_token.parse().expect("invalid collateral address");
    let selector = keccak256(b"splitPosition(address,bytes32,bytes32,uint256[],uint256)");

    let mut calldata = selector[..4].to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::Address(collateral),
        Token::FixedBytes(parent_collection.to_vec()),
        Token::FixedBytes(condition_id.to_vec()),
        Token::Array(partition.iter().map(|&i| Token::Uint(U256::from(i))).collect()),
        Token::Uint(amount),
    ]));

    Transaction {
        to: contracts::CTF.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Build a mergePositions(address,bytes32,bytes32,uint256[],uint256) transaction.
///
/// Merges outcome tokens back into collateral.
pub fn merge_positions(
    collateral_token: &str,
    parent_collection: [u8; 32],
    condition_id: [u8; 32],
    partition: &[u64],
    amount: U256,
) -> Transaction {
    let collateral: Address = collateral_token.parse().expect("invalid collateral address");
    let selector = keccak256(b"mergePositions(address,bytes32,bytes32,uint256[],uint256)");

    let mut calldata = selector[..4].to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::Address(collateral),
        Token::FixedBytes(parent_collection.to_vec()),
        Token::FixedBytes(condition_id.to_vec()),
        Token::Array(partition.iter().map(|&i| Token::Uint(U256::from(i))).collect()),
        Token::Uint(amount),
    ]));

    Transaction {
        to: contracts::CTF.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Convenience: split USDC.e with default parent collection.
pub fn split_regular(condition_id: [u8; 32], partition: &[u64], amount: U256) -> Transaction {
    split_position(contracts::USDC_E, [0u8; 32], condition_id, partition, amount)
}

/// Convenience: merge back to USDC.e with default parent collection.
pub fn merge_regular(condition_id: [u8; 32], partition: &[u64], amount: U256) -> Transaction {
    merge_positions(contracts::USDC_E, [0u8; 32], condition_id, partition, amount)
}
