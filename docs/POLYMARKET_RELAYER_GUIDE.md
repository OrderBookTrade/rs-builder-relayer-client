# How to Use the Polymarket Relayer SDK (Python & TypeScript)

Polymarket lets you trade on real-world events. When a market settles, you need to **redeem** your winning positions to get your USDC back. This article explains how to do that programmatically using the official Python or TypeScript SDKs — no gas required.

---

## What Is the Builder Relayer?

Normally, on-chain transactions on Polygon require you to pay gas (POL). Polymarket's **Builder Relayer** removes that friction: you sign a transaction with your private key, send it to Polymarket's relayer server, and the relayer submits it on-chain and pays the gas for you.

This enables:
- **Redeeming settled positions** (claim your winnings)
- **Token approvals** (one-time setup)
- **Splitting / merging** outcome tokens
- **Deploying** your Safe wallet

---

## Before You Start

You need:

1. **A Relayer API key** — go to [polymarket.com/settings](https://polymarket.com/settings) → API Keys → Create Key. Copy the key and the associated wallet address.
2. **Your EOA private key** — the signing key for your Polymarket wallet.
3. **A deployed Safe wallet** — if you haven't used the relayer before, run the setup step first (covered below).

---

## Option 1: Python SDK

### Install

```bash
pip install py-builder-relayer-client
```

Source: [github.com/Polymarket/py-builder-relayer-client](https://github.com/Polymarket/py-builder-relayer-client)

### Setup (deploy Safe + approvals)

Do this once before your first relayed transaction:

```python
from py_clob_client.client import ClobClient
from py_builder_relayer_client import RelayerClient, RelayerTxType

# Your credentials
PRIVATE_KEY = "0x..."
RELAYER_API_KEY = "your_api_key"
RELAYER_ADDRESS = "0xYourEOAAddress"

client = RelayerClient(
    key=PRIVATE_KEY,
    chain_id=137,  # Polygon mainnet
    relayer_api_key=RELAYER_API_KEY,
    relayer_api_key_address=RELAYER_ADDRESS,
    tx_type=RelayerTxType.SAFE,
)

# One-time: deploy Safe wallet
result = client.deploy()
print(f"Safe deployed at: {result.proxy_address}")

# One-time: approve USDC and tokens
client.approve(client.usdc, client.ctf_exchange_address)
client.approve(client.usdc, client.neg_risk_exchange_address)
client.set_approval_for_all(client.ctf, client.ctf_exchange_address)
client.set_approval_for_all(client.ctf, client.neg_risk_exchange_address)
client.set_approval_for_all(client.ctf, client.neg_risk_adapter_address)
```

### Redeem a Settled Position

```python
# condition_id comes from the CLOB API market data
condition_id = "0xabc123..."

# For regular markets:
result = client.redeem(
    condition_id=condition_id,
    index_sets=[1, 2],  # both outcomes
)
print(f"Redeemed! tx: {result.tx_hash}")

# For neg-risk markets (presidential elections, etc.):
result = client.redeem_neg_risk(
    condition_id=condition_id,
    index_sets=[1, 2],
)
print(f"Redeemed neg-risk! tx: {result.tx_hash}")
```

### Redeem All Settled Positions

```python
from py_clob_client.client import ClobClient

# Fetch your positions from CLOB API
clob = ClobClient(host="https://clob.polymarket.com", key=PRIVATE_KEY, chain_id=137)
positions = clob.get_positions()  # returns your open positions

# Filter to settled ones, then redeem each
for position in positions:
    if position.market.resolved:
        client.redeem(
            condition_id=position.condition_id,
            index_sets=[1, 2],
        )
        print(f"Redeemed {position.market.question}")
```

---

## Option 2: TypeScript SDK

### Install

```bash
npm install @polymarket/builder-relayer-client
# or
yarn add @polymarket/builder-relayer-client
```

Source: [github.com/Polymarket/builder-relayer-client](https://github.com/Polymarket/builder-relayer-client)

### Setup (deploy Safe + approvals)

```typescript
import { RelayerClient, RelayerTxType } from "@polymarket/builder-relayer-client";
import { ethers } from "ethers";

const wallet = new ethers.Wallet(process.env.PRIVATE_KEY!);

const client = new RelayerClient({
  signer: wallet,
  chainId: 137,
  relayerApiKey: process.env.POLY_RELAYER_API_KEY!,
  relayerApiKeyAddress: process.env.POLY_RELAYER_ADDRESS!,
  txType: RelayerTxType.SAFE,
});

// One-time: deploy Safe wallet
const deployResult = await client.deploy();
console.log(`Safe deployed: ${deployResult.proxyAddress}`);

// One-time: approve USDC and tokens
await client.approveUsdcForCtfExchange();
await client.approveUsdcForNegRiskExchange();
await client.approveCtfForCtfExchange();
await client.approveCtfForNegRiskExchange();
await client.approveCtfForNegRiskAdapter();
```

### Redeem a Settled Position

```typescript
// condition_id from the market data API
const conditionId = "0xabc123...";

// Regular market
const result = await client.redeemPositions(
  conditionId,
  [1, 2]  // both outcomes
);
console.log(`Redeemed! tx: ${result.txHash}`);

// Neg-risk market
const negResult = await client.redeemNegRiskPositions(
  conditionId,
  [1, 2]
);
console.log(`Neg-risk redeemed! tx: ${negResult.txHash}`);
```

### Batch Multiple Operations

The SDK supports batching multiple transactions atomically:

```typescript
import { buildApproveTransaction, buildRedeemTransaction } from "@polymarket/builder-relayer-client";

const txs = [
  buildApproveTransaction(usdcAddress, ctfExchange, ethers.MaxUint256),
  buildRedeemTransaction(conditionId, [1, 2]),
];

// Both succeed or both fail — atomically
const result = await client.execute(txs, "Approve + Redeem");
const confirmed = await result.wait();
console.log(`Done! State: ${confirmed.state}`);
```

---

## Getting the Condition ID

The `condition_id` for a market comes from the CLOB API:

**HTTP:**
```bash
curl "https://clob.polymarket.com/markets/MARKET_ID" | jq .condition_id
```

**Python:**
```python
market = clob.get_market("MARKET_ID")
condition_id = market.condition_id
```

**TypeScript:**
```typescript
const market = await clobClient.getMarket("MARKET_ID");
const conditionId = market.conditionId;
```

---

## Transaction States

After submitting, poll until you reach a terminal state:

| State | Meaning |
|---|---|
| `NEW` | Received by relayer |
| `EXECUTED` | Submitted to Polygon |
| `MINED` | Included in a block |
| `CONFIRMED` | Finalized ✅ |
| `FAILED` | Failed on-chain ❌ |
| `INVALID` | Rejected by relayer ❌ |

Both SDKs provide a `.wait()` method that polls automatically until `CONFIRMED` or error.

---

## Troubleshooting

**"Wallet not deployed" error**
→ Run `client.deploy()` first. Only needed once per key.

**"Token not approved" / reverted redemption**
→ Run the approval setup step. Only needed once.

**"API key invalid"**
→ Regenerate your key at polymarket.com/settings → API Keys.

**Neg-risk vs regular markets**
→ Check if the market's `neg_risk` field is `true`. Presidential elections, multi-outcome markets are typically neg-risk. Use `redeem_neg_risk` for those.

---

## Reference Links

| Resource | URL |
|---|---|
| Gasless docs | https://docs.polymarket.com/trading/gasless |
| Python SDK | https://github.com/Polymarket/py-builder-relayer-client |
| TypeScript SDK | https://github.com/Polymarket/builder-relayer-client |
| Python signing SDK | https://github.com/Polymarket/py-builder-signing-sdk |
| TS signing SDK | https://github.com/Polymarket/builder-signing-sdk |
| Contract addresses | https://docs.polymarket.com/resources/contract-addresses |
| CLOB API docs | https://docs.polymarket.com/trading/clob |
