use ethers::abi::{encode, Token};
use ethers::signers::Signer;
use ethers::types::{Address, H256, U256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::error::{RelayerError, Result};
use crate::types::{ProxySignatureParams, RelayPayload, Transaction};

/// Encode transactions for the proxy contract.
///
/// proxy(calls: (uint8, address, uint256, bytes)[])
/// Selector: keccak256("proxy((uint8,address,uint256,bytes)[])")[:4]
pub fn encode_proxy_calls(txs: &[Transaction]) -> Result<Vec<u8>> {
    let selector = &keccak256(b"proxy((uint8,address,uint256,bytes)[])")[..4];

    let mut call_tuples = Vec::new();
    for tx in txs {
        let to: Address = tx
            .to
            .parse()
            .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;
        let data = hex::decode(tx.data.strip_prefix("0x").unwrap_or(&tx.data))
            .map_err(|e| RelayerError::Abi(format!("Invalid hex data: {e}")))?;
        let value = U256::from_dec_str(&tx.value)
            .map_err(|e| RelayerError::Abi(format!("Invalid value: {e}")))?;

        call_tuples.push(Token::Tuple(vec![
            Token::Uint(U256::one()), // typeCode: 1 = Call (Polymarket proxy convention)
            Token::Address(to),
            Token::Uint(value),
            Token::Bytes(data),
        ]));
    }

    let encoded = encode(&[Token::Array(call_tuples)]);
    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&encoded);

    Ok(calldata)
}

/// Build a Proxy transaction: sign and return (data, signature, signatureParams).
pub async fn build_proxy_transaction<S: Signer>(
    signer: &S,
    signer_address: Address,
    txs: &[Transaction],
    relay_payload: &RelayPayload,
    gas_limit: u64,
) -> Result<(String, String, ProxySignatureParams)> {
    let proxy_factory: Address = contracts::PROXY_FACTORY
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;
    let relay_hub: Address = contracts::RELAY_HUB
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;
    let relay_addr: Address = relay_payload
        .address
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;

    let calldata = encode_proxy_calls(txs)?;
    let nonce = relay_payload
        .nonce
        .parse::<u64>()
        .map_err(|e| RelayerError::Other(format!("Invalid nonce: {e}")))?;

    // Build struct hash: keccak256(concat("rlx:", from, to, data, txFee, gasPrice, gasLimit, nonce, relayHub, relay))
    let mut packed = Vec::new();
    packed.extend_from_slice(b"rlx:");
    packed.extend_from_slice(signer_address.as_bytes());
    packed.extend_from_slice(proxy_factory.as_bytes());
    packed.extend_from_slice(&calldata);

    // txFee (32 bytes) = 0
    packed.extend_from_slice(&[0u8; 32]);
    // gasPrice (32 bytes) = 0
    packed.extend_from_slice(&[0u8; 32]);
    // gasLimit (32 bytes)
    let mut gl_bytes = [0u8; 32];
    U256::from(gas_limit).to_big_endian(&mut gl_bytes);
    packed.extend_from_slice(&gl_bytes);
    // nonce (32 bytes)
    let mut nonce_bytes = [0u8; 32];
    U256::from(nonce).to_big_endian(&mut nonce_bytes);
    packed.extend_from_slice(&nonce_bytes);
    // relayHub
    packed.extend_from_slice(relay_hub.as_bytes());
    // relay
    packed.extend_from_slice(relay_addr.as_bytes());

    let struct_hash = H256::from(keccak256(&packed));

    // Sign the hash
    let signature = signer
        .sign_message(struct_hash.as_bytes())
        .await
        .map_err(|e| RelayerError::Signing(e.to_string()))?;

    let sig_hex = format!("0x{}", hex::encode(signature.to_vec()));
    let data_hex = format!("0x{}", hex::encode(&calldata));

    let sig_params = ProxySignatureParams {
        gas_price: "0".to_string(),
        gas_limit: gas_limit.to_string(),
        relayer_fee: "0".to_string(),
        relay_hub: contracts::RELAY_HUB.to_string(),
        relay: relay_payload.address.clone(),
    };

    Ok((data_hex, sig_hex, sig_params))
}
