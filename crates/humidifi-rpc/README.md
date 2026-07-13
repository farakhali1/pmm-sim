# humidifi-rpc

Build & simulate **direct** PropAMM swaps over RPC for:

- **HumidiFi** (v1 / v2 / v3)
- **GoonFi**
- **SolFi** (v2)

Pick the venue with `"dex"` in `config.json`.

## Config

```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "./payer.json",
  "dex": "humidifi",
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
| `dex` | `humidifi` \| `goonfi` \| `solfi` |
| `version` | HumidiFi only: `v1` \| `v2` \| `v3` |
| `pool` | Market pubkey |
| `direction` | `0` = base→quote, `1` = quote→base |
| `base_mint` / `quote_mint` | Optional; select vaults when a market owns many TAs |

### SolFi example

```json
{
  "dex": "solfi",
  "pool": "65ZHSArs5XxPseKQbB1B4r16vDxMWnCxHMzogDAqiDUc",
  "base_mint": "So11111111111111111111111111111111111111112",
  "quote_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "cfg": "FmxXDSR9WvpJTCh738D1LEDuhMoA8geCtZgHb3isy7Dp",
  "oracle": "2ny7eGyZCoeEVTkNLf5HcnJFBKkyA4p4gcrtb3b8y8ou",
  "amount_in": 1000000,
  "direction": 0,
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "./payer.json"
}
```

`cfg` / `oracle` come from [`cfg/setup.toml`](../../cfg/setup.toml). If omitted, those same WSOL-USDC defaults are used. Vaults are still fetched from RPC.
### GoonFi example

```json
{
  "dex": "goonfi",
  "pool": "4uWuh9fC7rrZKrN8ZdJf69MN1e2S7FPpMqcsyY1aof6K",
  "amount_in": 1000000,
  "direction": 0,
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "./payer.json"
}
```

## Discovery

1. Vault TAs via `getTokenAccountsByOwner(pool)` when the market owns them
2. Extra accounts (`add1`/`vote`, SolFi `oracle`/`cfg`, GoonFi `blacklist`) from the latest live swap ix for that market
3. Fresh state via `getMultipleAccounts` before simulate

## Run

```bash
cargo run -p humidifi-rpc -- --config crates/humidifi-rpc/config.json
```

## Txns

### Humidifi V1:
  Txn: Ezi3nLfKhrFiKUYUGoZ7KtrTwf2pTxeEn1BMEKFiUhf5eKZ3docDb1i8pneG9HcXnXte1NE32QQ7Q53XWd6gKLJ
  Price:
  - Market rate: $76.6
  - Swap rate: $74.9