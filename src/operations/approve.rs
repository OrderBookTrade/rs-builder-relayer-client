use ethers::abi::encode;
use ethers::abi::Token;
use ethers::types::{Address, U256};

use crate::contracts;
use crate::types::Transaction;

/// Build an ERC20 approve(address spender, uint256 amount) transaction.
pub fn approve(token: &str, spender: &str, amount: U256) -> Transaction {
    let spender_addr: Address = spender.parse().expect("invalid spender address");
    let mut calldata = contracts::APPROVE_SELECTOR.to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::Address(spender_addr),
        Token::Uint(amount),
    ]));

    Transaction {
        to: token.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Build an ERC1155 setApprovalForAll(address operator, bool approved) transaction.
pub fn set_approval_for_all(token: &str, operator: &str, approved: bool) -> Transaction {
    let operator_addr: Address = operator.parse().expect("invalid operator address");
    let mut calldata = contracts::SET_APPROVAL_FOR_ALL_SELECTOR.to_vec();
    calldata.extend_from_slice(&encode(&[
        Token::Address(operator_addr),
        Token::Bool(approved),
    ]));

    Transaction {
        to: token.to_string(),
        data: format!("0x{}", hex::encode(&calldata)),
        value: "0".to_string(),
    }
}

/// Approve USDC.e for CTF Exchange (max allowance).
pub fn approve_usdc_for_ctf_exchange() -> Transaction {
    approve(contracts::USDC_E, contracts::CTF_EXCHANGE, U256::MAX)
}

/// Approve USDC.e for Neg Risk CTF Exchange (max allowance).
pub fn approve_usdc_for_neg_risk_exchange() -> Transaction {
    approve(contracts::USDC_E, contracts::NEG_RISK_EXCHANGE, U256::MAX)
}

/// Approve all CTF tokens (ERC1155) for CTF Exchange.
pub fn approve_ctf_for_ctf_exchange() -> Transaction {
    set_approval_for_all(contracts::CTF, contracts::CTF_EXCHANGE, true)
}

/// Approve all CTF tokens (ERC1155) for Neg Risk CTF Exchange.
pub fn approve_ctf_for_neg_risk_exchange() -> Transaction {
    set_approval_for_all(contracts::CTF, contracts::NEG_RISK_EXCHANGE, true)
}

/// Approve all CTF tokens (ERC1155) for Neg Risk Adapter.
pub fn approve_ctf_for_neg_risk_adapter() -> Transaction {
    set_approval_for_all(contracts::CTF, contracts::NEG_RISK_ADAPTER, true)
}
