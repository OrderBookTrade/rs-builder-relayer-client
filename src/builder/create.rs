use ethers::abi::{encode, Token};
use ethers::signers::Signer;
use ethers::types::{Address, U256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::error::{RelayerError, Result};
use crate::types::CreateSignatureParams;

/// Build a Safe-Create transaction for deploying a new Safe wallet.
pub async fn build_create_transaction<S: Signer>(
    signer: &S,
    chain_id: u64,
) -> Result<(String, CreateSignatureParams)> {
    let safe_factory: Address = contracts::SAFE_FACTORY
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;

    let params = CreateSignatureParams::default();

    // Domain type hash
    let domain_type_hash = keccak256(
        b"EIP712Domain(string name,uint256 chainId,address verifyingContract)",
    );

    // Domain separator
    let name_hash = keccak256(b"Polymarket Contract Proxy Factory");
    let domain_sep = keccak256(encode(&[
        Token::FixedBytes(domain_type_hash.to_vec()),
        Token::FixedBytes(name_hash.to_vec()),
        Token::Uint(U256::from(chain_id)),
        Token::Address(safe_factory),
    ]));

    // Struct type hash
    let type_hash = keccak256(
        b"CreateProxy(address paymentToken,uint256 payment,address paymentReceiver)",
    );

    // Struct hash
    let struct_hash = keccak256(encode(&[
        Token::FixedBytes(type_hash.to_vec()),
        Token::Address(Address::zero()), // paymentToken
        Token::Uint(U256::zero()),       // payment
        Token::Address(Address::zero()), // paymentReceiver
    ]));

    // EIP-712 hash
    let mut eip712_data = Vec::with_capacity(66);
    eip712_data.extend_from_slice(&[0x19, 0x01]);
    eip712_data.extend_from_slice(&domain_sep);
    eip712_data.extend_from_slice(&struct_hash);
    let msg_hash = keccak256(&eip712_data);

    // Sign
    let signature = signer
        .sign_message(&msg_hash[..])
        .await
        .map_err(|e| RelayerError::Signing(e.to_string()))?;

    let sig_hex = format!("0x{}", hex::encode(signature.to_vec()));

    Ok((sig_hex, params))
}
