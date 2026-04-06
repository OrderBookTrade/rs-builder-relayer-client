use ethers::types::Address;
use std::str::FromStr;

// Polygon Mainnet (Chain ID 137)

/// USDC.e on Polygon
pub const USDC_E: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";

/// Conditional Tokens Framework
pub const CTF: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";

/// CTF Exchange
pub const CTF_EXCHANGE: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";

/// Neg Risk CTF Exchange
pub const NEG_RISK_EXCHANGE: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";

/// Neg Risk Adapter
pub const NEG_RISK_ADAPTER: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";

/// Proxy Factory (for Proxy wallet type)
pub const PROXY_FACTORY: &str = "0xaB45c5A4B0c941a2F231C04C3f49182e1A254052";

/// Safe Factory (for Safe wallet type)
pub const SAFE_FACTORY: &str = "0xaacFeEa03eb1561C4e67d661e40682Bd20E3541b";

/// Safe Multisend contract
pub const SAFE_MULTISEND: &str = "0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761";

/// Relay Hub (for Proxy transactions)
pub const RELAY_HUB: &str = "0xD216153c06E857cD7f72665E0aF1d7D82172F494";

/// Default relayer URL
pub const RELAYER_URL: &str = "https://relayer-v2.polymarket.com/";

/// Zero address
pub const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Safe init code hash for CREATE2 derivation
pub const SAFE_INIT_CODE_HASH: &str =
    "0x2bce2127ff07fb632d16c8347c4ebf501f4841168bed00d9e6ef715ddb6fcecf";

/// Proxy init code hash for CREATE2 derivation
pub const PROXY_INIT_CODE_HASH: &str =
    "0xd21df8dc65880a8606f09fe0ce3df9b8869287ab0b058be05aa9e8af6330a00b";

/// Parse a hex address string to an ethers Address.
pub fn parse_address(s: &str) -> crate::error::Result<Address> {
    Address::from_str(s).map_err(|e| crate::error::RelayerError::InvalidAddress(e.to_string()))
}

// ── ABI Function Selectors ──

/// ERC20 approve(address,uint256) selector
pub const APPROVE_SELECTOR: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];

/// ERC1155 setApprovalForAll(address,bool) selector
pub const SET_APPROVAL_FOR_ALL_SELECTOR: [u8; 4] = [0xa2, 0x2c, 0xb4, 0x65];

/// ConditionalTokens redeemPositions(address,bytes32,bytes32,uint256[]) selector
pub const REDEEM_POSITIONS_SELECTOR: [u8; 4] = [0x01, 0xb7, 0x03, 0x7c]; // Will compute

/// ConditionalTokens splitPosition(address,bytes32,bytes32,uint256[],uint256) selector
pub const SPLIT_POSITION_SELECTOR: [u8; 4] = [0x72, 0xce, 0x42, 0x75]; // Will compute

/// ConditionalTokens mergePositions(address,bytes32,bytes32,uint256[],uint256) selector
pub const MERGE_POSITIONS_SELECTOR: [u8; 4] = [0xd3, 0x7b, 0xf4, 0x2e]; // Will compute

/// Safe multiSend(bytes) selector
pub const MULTISEND_SELECTOR: [u8; 4] = [0x8d, 0x80, 0xff, 0x0a];
