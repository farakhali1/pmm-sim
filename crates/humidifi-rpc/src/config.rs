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

/// Runtime config loaded from `config.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub keypair_path: String,
    /// Path to pmm-sim `cfg/setup.toml` (HumidiFi market catalog).
    pub setup_path: String,
    /// HumidiFi market / pool pubkey.
    pub pool: String,
    pub version: SwapVersion,
    /// Amount in raw token units.
    pub amount_in: u64,
    /// `0` = base→quote, `1` = quote→base (same as pmm-sim HumidiFi).
    pub direction: u8,
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

    pub fn pool_pubkey(&self) -> eyre::Result<Pubkey> {
        Pubkey::from_str(&self.pool).map_err(|e| eyre::eyre!("invalid pool pubkey '{}': {e}", self.pool))
    }
}
