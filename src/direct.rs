//! Direct on-chain execution via Gnosis Safe — fallback when relayer quota is exhausted.
//!
//! Flow (matches the Python script's `redeem_direct`):
//! 1. Read Safe nonce via `nonce()` view call
//! 2. Get Safe transaction hash via `getTransactionHash()` on-chain call
//! 3. ECDSA-sign the raw hash (no eth_sign prefix)
//! 4. Pack signature r + s + v (v = 27 or 28)
//! 5. Call `execTransaction` on the Safe

use ethers::abi::{encode, Token};
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{
    Address, Bytes, Eip1559TransactionRequest, H256, TransactionReceipt, U256,
};
use ethers::utils::keccak256;

use crate::builder::derive::derive_safe_address;
use crate::error::{RelayerError, Result};
use crate::types::Transaction;

const DEFAULT_GAS_LIMIT: u64 = 500_000;

/// Result of a direct on-chain redemption.
#[derive(Debug)]
pub struct DirectTxResult {
    pub tx_hash: String,
    pub success: bool,
    pub gas_used: u64,
    pub gas_cost_matic: f64,
    pub block_number: u64,
}

/// Executor for direct on-chain Safe transactions (no relayer).
pub struct DirectExecutor {
    provider: SignerMiddleware<Provider<Http>, LocalWallet>,
    signer_address: Address,
    safe_address: Address,
    chain_id: u64,
}

impl DirectExecutor {
    /// Create a new DirectExecutor.
    pub fn new(rpc_url: &str, signer: LocalWallet, chain_id: u64) -> Result<Self> {
        let signer_address = signer.address();
        let safe_address = derive_safe_address(signer_address)?;

        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| RelayerError::Other(format!("Invalid RPC URL: {e}")))?;
        let provider = SignerMiddleware::new(provider, signer.with_chain_id(chain_id));

        Ok(Self {
            provider,
            signer_address,
            safe_address,
            chain_id,
        })
    }

    pub fn safe_address(&self) -> Address {
        self.safe_address
    }

    pub fn signer_address(&self) -> Address {
        self.signer_address
    }

    /// Get MATIC balance of the EOA (for gas).
    pub async fn get_matic_balance(&self) -> Result<f64> {
        let balance = self
            .provider
            .get_balance(self.signer_address, None)
            .await
            .map_err(|e| RelayerError::Other(format!("Failed to get balance: {e}")))?;
        let matic = balance.as_u128() as f64 / 1e18;
        Ok(matic)
    }

    /// Execute a transaction directly through the Gnosis Safe.
    pub async fn execute(&self, tx: &Transaction) -> Result<DirectTxResult> {
        let target: Address = tx
            .to
            .parse()
            .map_err(|e: <Address as std::str::FromStr>::Err| {
                RelayerError::InvalidAddress(e.to_string())
            })?;
        let inner_calldata = hex::decode(tx.data.strip_prefix("0x").unwrap_or(&tx.data))
            .map_err(|e| RelayerError::Abi(format!("Invalid calldata hex: {e}")))?;

        // 1. Read Safe nonce
        let safe_nonce = self.read_safe_nonce().await?;
        tracing::debug!(safe_nonce, "Safe nonce");

        // 2. Get the Safe tx hash from the contract itself (authoritative)
        let safe_tx_hash = self
            .get_transaction_hash_onchain(target, &inner_calldata, safe_nonce)
            .await?;
        tracing::debug!(hash = ?safe_tx_hash, "Safe tx hash from contract");

        // 3. ECDSA-sign the raw hash (NO eth_sign prefix — raw sign_hash)
        let signature = self
            .provider
            .signer()
            .sign_hash(safe_tx_hash)
            .map_err(|e| RelayerError::Signing(e.to_string()))?;

        // 4. Pack signature: r(32) + s(32) + v(1), v = 27 or 28
        let mut packed_sig = Vec::with_capacity(65);
        let mut r_bytes = [0u8; 32];
        signature.r.to_big_endian(&mut r_bytes);
        packed_sig.extend_from_slice(&r_bytes);
        let mut s_bytes = [0u8; 32];
        signature.s.to_big_endian(&mut s_bytes);
        packed_sig.extend_from_slice(&s_bytes);
        packed_sig.push(signature.v as u8);

        // 5. Build execTransaction calldata
        let exec_calldata =
            self.encode_exec_transaction(target, &inner_calldata, &packed_sig);

        // 6. Send transaction
        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .map_err(|e| RelayerError::Other(format!("Failed to get gas price: {e}")))?;

        let tx_request = Eip1559TransactionRequest::new()
            .to(self.safe_address)
            .data(exec_calldata)
            .gas(DEFAULT_GAS_LIMIT)
            .max_fee_per_gas(gas_price * 3 / 2)
            .max_priority_fee_per_gas(U256::from(30_000_000_000u64)); // 30 gwei

        let pending = self
            .provider
            .send_transaction(tx_request, None)
            .await
            .map_err(|e| RelayerError::Other(format!("Failed to send tx: {e}")))?;

        let tx_hash = format!("{:?}", pending.tx_hash());
        tracing::info!(tx_hash = %tx_hash, "Direct tx sent");

        let receipt: TransactionReceipt = pending
            .await
            .map_err(|e| RelayerError::Other(format!("Tx failed: {e}")))?
            .ok_or_else(|| RelayerError::Other("No receipt".to_string()))?;

        let gas_used = receipt.gas_used.map(|g| g.as_u64()).unwrap_or(0);
        let effective_gas_price = receipt
            .effective_gas_price
            .map(|p| p.as_u128())
            .unwrap_or(0);
        let gas_cost_matic = gas_used as f64 * effective_gas_price as f64 / 1e18;
        let block_number = receipt.block_number.map(|b| b.as_u64()).unwrap_or(0);
        let success = receipt.status.map(|s| s.as_u64() == 1).unwrap_or(false);

        if success {
            tracing::info!(block = block_number, gas = gas_used, "Direct tx confirmed");
        } else {
            tracing::warn!(tx_hash = %tx_hash, "Direct tx reverted");
        }

        Ok(DirectTxResult {
            tx_hash,
            success,
            gas_used,
            gas_cost_matic,
            block_number,
        })
    }

    // ── On-chain calls ──────────────────────────────────────────────────

    /// Read the Safe nonce via eth_call to `nonce()`.
    async fn read_safe_nonce(&self) -> Result<u64> {
        let selector = &keccak256(b"nonce()")[..4];
        let result = self.eth_call_safe(selector).await?;
        if result.len() < 32 {
            return Err(RelayerError::Other("Invalid nonce response".to_string()));
        }
        Ok(U256::from_big_endian(&result[..32]).as_u64())
    }

    /// Get the Safe tx hash via eth_call to `getTransactionHash(...)`.
    ///
    /// This is the authoritative hash the Safe uses for signature verification.
    /// Matches Python's `create_struct_hash()` which calls the same view function.
    async fn get_transaction_hash_onchain(
        &self,
        to: Address,
        data: &[u8],
        nonce: u64,
    ) -> Result<H256> {
        let selector = &keccak256(
            b"getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)",
        )[..4];

        let encoded_args = encode(&[
            Token::Address(to),
            Token::Uint(U256::zero()),           // value
            Token::Bytes(data.to_vec()),         // data
            Token::Uint(U256::zero()),           // operation = Call
            Token::Uint(U256::zero()),           // safeTxGas
            Token::Uint(U256::zero()),           // baseGas
            Token::Uint(U256::zero()),           // gasPrice
            Token::Address(Address::zero()),     // gasToken
            Token::Address(Address::zero()),     // refundReceiver
            Token::Uint(U256::from(nonce)),      // _nonce
        ]);

        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded_args);

        let result = self.eth_call_safe(&calldata).await?;
        if result.len() < 32 {
            return Err(RelayerError::Other(
                "Invalid getTransactionHash response".to_string(),
            ));
        }
        Ok(H256::from_slice(&result[..32]))
    }

    /// Helper: eth_call to the Safe contract.
    async fn eth_call_safe(&self, calldata: &[u8]) -> Result<Bytes> {
        self.provider
            .call(
                &ethers::types::transaction::eip2718::TypedTransaction::Eip1559(
                    Eip1559TransactionRequest::new()
                        .to(self.safe_address)
                        .data(Bytes::from(calldata.to_vec())),
                ),
                None,
            )
            .await
            .map_err(|e| RelayerError::Other(format!("eth_call failed: {e}")))
    }

    /// Encode execTransaction calldata.
    fn encode_exec_transaction(
        &self,
        to: Address,
        inner_data: &[u8],
        signature: &[u8],
    ) -> Vec<u8> {
        let selector = &keccak256(
            b"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
        )[..4];

        let encoded = encode(&[
            Token::Address(to),
            Token::Uint(U256::zero()),
            Token::Bytes(inner_data.to_vec()),
            Token::Uint(U256::zero()),           // operation
            Token::Uint(U256::zero()),           // safeTxGas
            Token::Uint(U256::zero()),           // baseGas
            Token::Uint(U256::zero()),           // gasPrice
            Token::Address(Address::zero()),     // gasToken
            Token::Address(Address::zero()),     // refundReceiver
            Token::Bytes(signature.to_vec()),
        ]);

        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);
        calldata
    }
}
