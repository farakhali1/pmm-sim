# humidifi-rpc

Sample crate that builds a **direct** HumidiFi swap instruction (v1 / v2 / v3) from `cfg/setup.toml`, fetches the required accounts over RPC, and **simulates** the signed transaction (does not send).

## Config (`config.json`)

```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "./payer.json",
  "setup_path": "./cfg/setup.toml",
  "pool": "FksffEqnBRixYGR791Qw2MgdU7zNCpHVFYBL4Fa4qVuH",
  "version": "v3",
  "amount_in": 1000000,
  "direction": 0
}
```

Paths (`keypair_path`, `setup_path`) are relative to the **current working directory** (run from the repo root).

| Field | Meaning |
| --- | --- |
| `keypair_path` | Solana keypair JSON (fee payer + swap authority) |
| `pool` | HumidiFi market pubkey (must exist under the chosen version in `setup.toml`) |
| `version` | `v1`, `v2`, or `v3` |
| `amount_in` | Raw token amount |
| `direction` | `0` = base→quote, `1` = quote→base |

Copy the example and edit paths:

```bash
cp crates/humidifi-rpc/config.example.json crates/humidifi-rpc/config.json
```

## Run

From the repo root:

```bash
cargo run -p humidifi-rpc -- --config crates/humidifi-rpc/config.json
```

## What it does

1. Loads `config.json` + HumidiFi markets from `setup_path`
2. Derives user base/quote ATAs for the payer
3. Fetches pool + user accounts via `getMultipleAccounts`
4. Builds a direct HumidiFi program ix (`build_humidifi_v1_ix` / `v2` / `v3`)
5. Signs and calls `simulateTransaction` — prints CU, logs, and err
