use ethers::abi::{encode, Token};
use ethers::signers::Signer;
use ethers::types::{Address, H256, U256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::error::{RelayerError, Result};
use crate::types::{SafeSignatureParams, Transaction};

/// EIP-712 type hash for SafeTx.
/// keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")
fn safe_tx_type_hash() -> H256 {
    H256::from(keccak256(
        b"SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)",
    ))
}

/// EIP-712 domain separator for a Safe wallet.
/// keccak256(abi.encode(DOMAIN_SEPARATOR_TYPEHASH, chainId, safeAddress))
fn domain_separator(chain_id: u64, safe_address: Address) -> H256 {
    let domain_type_hash = H256::from(keccak256(
        b"EIP712Domain(uint256 chainId,address verifyingContract)",
    ));
    let encoded = encode(&[
        Token::FixedBytes(domain_type_hash.as_bytes().to_vec()),
        Token::Uint(U256::from(chain_id)),
        Token::Address(safe_address),
    ]);
    H256::from(keccak256(encoded))
}

/// Encode transactions for multisend.
/// Each tx is packed as: [uint8 operation, address to, uint256 value, uint256 dataLen, bytes data]
pub fn encode_multisend(txs: &[Transaction]) -> Result<Vec<u8>> {
    let mut packed = Vec::new();

    for tx in txs {
        let to: Address = tx
            .to
            .parse()
            .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;
        let data = hex::decode(tx.data.strip_prefix("0x").unwrap_or(&tx.data))
            .map_err(|e| RelayerError::Abi(format!("Invalid hex data: {e}")))?;
        let value = U256::from_dec_str(&tx.value)
            .map_err(|e| RelayerError::Abi(format!("Invalid value: {e}")))?;

        // operation: uint8 (1 byte) — always 0 (Call)
        packed.push(0u8);
        // to: address (20 bytes)
        packed.extend_from_slice(to.as_bytes());
        // value: uint256 (32 bytes)
        let mut value_bytes = [0u8; 32];
        value.to_big_endian(&mut value_bytes);
        packed.extend_from_slice(&value_bytes);
        // dataLength: uint256 (32 bytes)
        let mut len_bytes = [0u8; 32];
        U256::from(data.len()).to_big_endian(&mut len_bytes);
        packed.extend_from_slice(&len_bytes);
        // data: bytes
        packed.extend_from_slice(&data);
    }

    Ok(packed)
}

/// Build a Safe transaction: sign and return (data, signature, signatureParams).
///
/// For single tx: direct call to the target.
/// For multiple txs: multisend delegatecall.
pub async fn build_safe_transaction<S: Signer>(
    signer: &S,
    chain_id: u64,
    safe_address: Address,
    txs: &[Transaction],
    nonce: u64,
) -> Result<(String, Address, String, SafeSignatureParams)> {
    let (to, data_bytes, operation) = if txs.len() == 1 {
        let tx = &txs[0];
        let to: Address = tx
            .to
            .parse()
            .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;
        let data = hex::decode(tx.data.strip_prefix("0x").unwrap_or(&tx.data))
            .map_err(|e| RelayerError::Abi(format!("Invalid hex data: {e}")))?;
        (to, data, 0u8) // Call
    } else {
        // Multisend: encode all txs, wrap in multiSend(bytes) call
        let multisend_data = encode_multisend(txs)?;

        // multiSend(bytes) = 0x8d80ff0a + abi_encode(bytes)
        let encoded_call = encode(&[Token::Bytes(multisend_data)]);
        let mut calldata = contracts::MULTISEND_SELECTOR.to_vec();
        calldata.extend_from_slice(&encoded_call);

        let multisend_addr: Address = contracts::SAFE_MULTISEND
            .parse()
            .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;

        (multisend_addr, calldata, 1u8) // DelegateCall
    };

    let sig_params = SafeSignatureParams {
        operation: operation.to_string(),
        ..Default::default()
    };

    // Build the EIP-712 struct hash
    let data_hash = H256::from(keccak256(&data_bytes));
    let struct_hash = H256::from(keccak256(encode(&[
        Token::FixedBytes(safe_tx_type_hash().as_bytes().to_vec()),
        Token::Address(to),
        Token::Uint(U256::zero()), // value
        Token::FixedBytes(data_hash.as_bytes().to_vec()),
        Token::Uint(U256::from(operation)),
        Token::Uint(U256::zero()), // safeTxGas
        Token::Uint(U256::zero()), // baseGas
        Token::Uint(U256::zero()), // gasPrice
        Token::Address(Address::zero()), // gasToken
        Token::Address(Address::zero()), // refundReceiver
        Token::Uint(U256::from(nonce)),
    ])));

    // EIP-712 hash: keccak256("\x19\x01" + domainSeparator + structHash)
    let domain_sep = domain_separator(chain_id, safe_address);
    let mut eip712_data = Vec::with_capacity(66);
    eip712_data.extend_from_slice(&[0x19, 0x01]);
    eip712_data.extend_from_slice(domain_sep.as_bytes());
    eip712_data.extend_from_slice(struct_hash.as_bytes());
    let msg_hash = H256::from(keccak256(&eip712_data));

    // Sign the hash
    let signature = signer
        .sign_message(msg_hash.as_bytes())
        .await
        .map_err(|e| RelayerError::Signing(e.to_string()))?;

    // Adjust v value for Safe: v=27->31, v=28->32
    let v = signature.v as u8;
    let adjusted_v = if v == 0 || v == 1 {
        v + 31
    } else if v == 27 || v == 28 {
        v + 4
    } else {
        v
    };

    // Pack signature: r (32 bytes) + s (32 bytes) + v (1 byte)
    let mut sig_bytes = Vec::with_capacity(65);
    let mut r_bytes = [0u8; 32];
    signature.r.to_big_endian(&mut r_bytes);
    sig_bytes.extend_from_slice(&r_bytes);
    let mut s_bytes = [0u8; 32];
    signature.s.to_big_endian(&mut s_bytes);
    sig_bytes.extend_from_slice(&s_bytes);
    sig_bytes.push(adjusted_v);

    let sig_hex = format!("0x{}", hex::encode(&sig_bytes));
    let data_hex = format!("0x{}", hex::encode(&data_bytes));

    Ok((data_hex, to, sig_hex, sig_params))
}
