# humidifi-rpc

Self-contained sample: discover HumidiFi pool accounts over RPC, build a **direct** swap ix (v1 / v2 / v3), fetch those accounts, and **simulate** (no send).

## Config (`config.json`)

```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "./payer.json",
  "version": "v3",
  "amount_in": 1000000,
  "direction": 0,
  "pool": "FksffEqnBRixYGR791Qw2MgdU7zNCpHVFYBL4Fa4qVuH",
  "base_mint": "So11111111111111111111111111111111111111112",
  "quote_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
}
```

| Field | Meaning |
| --- | --- |
| `pool` | HumidiFi market pubkey (owns the vault token accounts) |
| `base_mint` / `quote_mint` | Optional; select which vaults when the market owns many TAs |
| `version` | `v1`, `v2`, or `v3` |
| `direction` | `0` = base→quote, `1` = quote→base |

## Account discovery

1. **Vaults** — `getTokenAccountsByOwner(pool)` (market is the vault authority)
2. **v2/v3 `add1` + `vote`** — scanned from the latest successful HumidiFi swap ix for that market (`add1` changes every swap; `vote` is consistently Jito1)
3. **Fresh state** — `getMultipleAccounts` for pool + user ATAs before simulate

## Why `vote` is Jito1

`J1to1yufRnoWn81KYg1XkTWzmKjnYSnmE2VY8DGUJ9Qv` is the **Jito1** validator vote account. Live HumidiFi v2/v3 swaps on these pools pass it as a readonly account. It is not an arbitrary placeholder: recent on-chain swaps keep using this same vote account while `add1` rotates. Arbitrary other vote accounts are unlikely to work if the program checks that account (e.g. leader/slot timing tied to that validator). Some older static configs used `11111111…` as a stub for unused markets, but successful mainnet swaps use Jito1.

## Run

```bash
cargo run -p humidifi-rpc -- --config crates/humidifi-rpc/config.json
```



## Txns

### V1:
  Txn: Ezi3nLfKhrFiKUYUGoZ7KtrTwf2pTxeEn1BMEKFiUhf5eKZ3docDb1i8pneG9HcXnXte1NE32QQ7Q53XWd6gKLJ
  Price:
  - Market rate: $76.6
  - Swap rate: $74.9