use ethers::abi::encode;
use ethers::abi::Token;
use ethers::types::{Address, U256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::types::Transaction;

/// Build a redeemPositions(address,bytes32,bytes32,uint256[]) transaction on the CTF contract.
///
/// # Arguments
/// * `collateral_token` - The collateral token address (USDC.e)
/// * `parent_collection` - Parent collection ID (usually bytes32(0))
/// * `condition_id` - The condition ID from market data
/// * `index_sets` - Index sets to redeem (e.g., [1, 2] for both outcomes)
pub fn redeem_positions(
    collateral_token: &str,
    parent_collection: [u8; 32],
    condition_id: [u8; 32],
    index_sets: &[u64],
) -> Transaction {
    let collateral: Address = collateral_token.parse().expect("invalid collateral address");
    let selector = keccak256(b"redeemPositions(address,bytes32,bytes32,uint256[])");

    let mut calldata = selector[..4].to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::Address(collateral),
        Token::FixedBytes(parent_collection.to_vec()),
        Token::FixedBytes(condition_id.to_vec()),
        Token::Array(index_sets.iter().map(|&i| Token::Uint(U256::from(i))).collect()),
    ]));

    Transaction {
        to: contracts::CTF.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Build a redeemPositions call for neg-risk markets via the NegRiskAdapter.
///
/// Uses the NegRiskAdapter contract instead of CTF directly.
pub fn redeem_neg_risk_positions(
    condition_id: [u8; 32],
    index_sets: &[u64],
) -> Transaction {
    let selector = keccak256(b"redeemPositions(bytes32,uint256[])");

    let mut calldata = selector[..4].to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::FixedBytes(condition_id.to_vec()),
        Token::Array(index_sets.iter().map(|&i| Token::Uint(U256::from(i))).collect()),
    ]));

    Transaction {
        to: contracts::NEG_RISK_ADAPTER.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Convenience: redeem a regular (non-neg-risk) position with default parent collection (0x00..00).
pub fn redeem_regular(condition_id: [u8; 32], index_sets: &[u64]) -> Transaction {
    redeem_positions(contracts::USDC_E, [0u8; 32], condition_id, index_sets)
}
