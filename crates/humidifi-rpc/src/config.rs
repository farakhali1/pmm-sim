use std::{fs, path::Path, str::FromStr};

use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwapVersion {
    V1,
    V2,
    V3,
}

impl FromStr for SwapVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "v1" | "1" => Ok(Self::V1),
            "v2" | "2" => Ok(Self::V2),
            "v3" | "3" => Ok(Self::V3),
            other => Err(format!("unknown HumidiFi version '{other}' (expected v1|v2|v3)")),
        }
    }
}

impl std::fmt::Display for SwapVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1 => write!(f, "v1"),
            Self::V2 => write!(f, "v2"),
            Self::V3 => write!(f, "v3"),
        }
    }
}

fn deser_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Pubkey::from_str(&s).map_err(serde::de::Error::custom)
}

fn deser_opt_pubkey<'de, D>(deserializer: D) -> Result<Option<Pubkey>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => Pubkey::from_str(&s).map(Some).map_err(serde::de::Error::custom),
    }
}

/// Resolved pool accounts used to build the swap ix (discovered via RPC).
#[derive(Debug, Clone)]
pub struct PoolAccounts {
    pub market: Pubkey,
    pub base_ta: Pubkey,
    pub quote_ta: Pubkey,
    pub token0_mint: Pubkey,
    pub token1_mint: Pubkey,
    /// Dynamic writable account taken from the latest live HumidiFi swap (v2/v3).
    pub add1: Pubkey,
    /// Validator vote account — live swaps use Jito1.
    pub vote: Pubkey,
}

impl PoolAccounts {
    pub fn account_pubkeys(&self, version: SwapVersion) -> Vec<Pubkey> {
        match version {
            SwapVersion::V1 => vec![self.market, self.base_ta, self.quote_ta],
            SwapVersion::V2 | SwapVersion::V3 => {
                vec![
                    self.market,
                    self.base_ta,
                    self.quote_ta,
                    self.token0_mint,
                    self.token1_mint,
                    self.add1,
                    self.vote,
                ]
            }
        }
    }
}

/// Runtime config loaded from `config.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub keypair_path: String,
    pub version: SwapVersion,
    /// Amount in raw token units.
    pub amount_in: u64,
    /// `0` = base→quote, `1` = quote→base.
    pub direction: u8,
    /// HumidiFi market / pool pubkey (vault owner).
    #[serde(deserialize_with = "deser_pubkey")]
    pub pool: Pubkey,
    /// Optional: select which vault mints to use when the market owns multiple TAs.
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub base_mint: Option<Pubkey>,
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub quote_mint: Option<Pubkey>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .map_err(|e| eyre::eyre!("failed to read config {}: {e}", path.display()))?;
        let cfg: Self = serde_json::from_str(&contents)
            .map_err(|e| eyre::eyre!("failed to parse config {}: {e}", path.display()))?;
        if cfg.direction > 1 {
            eyre::bail!("direction must be 0 (base→quote) or 1 (quote→base), got {}", cfg.direction);
        }
        Ok(cfg)
    }
}
