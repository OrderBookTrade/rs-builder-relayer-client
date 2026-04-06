use ethers::types::{Address, H256};
use ethers::utils::keccak256;

use crate::contracts;
use crate::error::{RelayerError, Result};

/// Compute CREATE2 address from a deployer, salt, and init code **hash**.
///
/// Formula: keccak256(0xff ++ deployer ++ salt ++ init_code_hash)[12..]
fn create2_address(deployer: Address, salt: H256, init_code_hash: H256) -> Address {
    let mut buf = Vec::with_capacity(1 + 20 + 32 + 32);
    buf.push(0xff);
    buf.extend_from_slice(deployer.as_bytes());
    buf.extend_from_slice(salt.as_bytes());
    buf.extend_from_slice(init_code_hash.as_bytes());
    let hash = keccak256(&buf);
    Address::from_slice(&hash[12..])
}

/// Derive the Safe wallet address for a signer using CREATE2.
///
/// salt = keccak256(abi_encode(signer_address))  (padded to 32 bytes)
pub fn derive_safe_address(signer: Address) -> Result<Address> {
    let factory: Address = contracts::SAFE_FACTORY
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;

    let init_code_hash: H256 = contracts::SAFE_INIT_CODE_HASH
        .parse()
        .map_err(|e: <H256 as std::str::FromStr>::Err| RelayerError::Other(e.to_string()))?;

    // ABI encode the address (left-padded to 32 bytes)
    let mut encoded = [0u8; 32];
    encoded[12..32].copy_from_slice(signer.as_bytes());
    let salt = H256::from(keccak256(encoded));

    Ok(create2_address(factory, salt, init_code_hash))
}

/// Derive the Proxy wallet address for a signer using CREATE2.
///
/// salt = keccak256(encode_packed(signer_address))  (20 bytes, not padded)
pub fn derive_proxy_address(signer: Address) -> Result<Address> {
    let factory: Address = contracts::PROXY_FACTORY
        .parse()
        .map_err(|e: <Address as std::str::FromStr>::Err| RelayerError::InvalidAddress(e.to_string()))?;

    let init_code_hash: H256 = contracts::PROXY_INIT_CODE_HASH
        .parse()
        .map_err(|e: <H256 as std::str::FromStr>::Err| RelayerError::Other(e.to_string()))?;

    // encodePacked: just the 20-byte address, no padding
    let salt = H256::from(keccak256(signer.as_bytes()));

    Ok(create2_address(factory, salt, init_code_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_safe_address() {
        let signer: Address = "0x6e0c80c90ea6c15917308F820Eac91Ce2724B5b5"
            .parse()
            .unwrap();
        // Actual derived address from CREATE2 with current SAFE_FACTORY + SAFE_INIT_CODE_HASH
        let derived = derive_safe_address(signer).unwrap();
        // Must be non-zero and deterministic
        assert_ne!(derived, Address::zero());
        assert_eq!(derived, derive_safe_address(signer).unwrap());
    }
}
