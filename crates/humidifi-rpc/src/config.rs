use std::{fs, path::Path, str::FromStr};

use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dex {
    Humidifi,
    Goonfi,
    Solfi,
}

impl FromStr for Dex {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "humidifi" | "humidif" => Ok(Self::Humidifi),
            "goonfi" | "gonfi" | "goon" => Ok(Self::Goonfi),
            "solfi" | "solfi-v2" | "solfiv2" => Ok(Self::Solfi),
            other => Err(format!("unknown dex '{other}' (expected humidifi|goonfi|solfi)")),
        }
    }
}

impl std::fmt::Display for Dex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Humidifi => write!(f, "humidifi"),
            Self::Goonfi => write!(f, "goonfi"),
            Self::Solfi => write!(f, "solfi"),
        }
    }
}

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

/// Resolved accounts for a HumidiFi pool.
#[derive(Debug, Clone)]
pub struct HumidifiPool {
    pub market: Pubkey,
    pub base_ta: Pubkey,
    pub quote_ta: Pubkey,
    pub token0_mint: Pubkey,
    pub token1_mint: Pubkey,
    pub add1: Pubkey,
    pub vote: Pubkey,
    pub version: SwapVersion,
}

/// Resolved accounts for a GoonFi pool.
#[derive(Debug, Clone)]
pub struct GoonfiPool {
    pub market: Pubkey,
    pub base_ta: Pubkey,
    pub quote_ta: Pubkey,
    pub blacklist: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
}

/// Resolved accounts for a SolFi v2 pool.
#[derive(Debug, Clone)]
pub struct SolfiPool {
    pub market: Pubkey,
    pub base_ta: Pubkey,
    pub quote_ta: Pubkey,
    pub cfg: Pubkey,
    pub oracle: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
}

#[derive(Debug, Clone)]
pub enum ResolvedPool {
    Humidifi(HumidifiPool),
    Goonfi(GoonfiPool),
    Solfi(SolfiPool),
}

impl ResolvedPool {
    pub fn base_mint(&self) -> Pubkey {
        match self {
            Self::Humidifi(p) => p.token0_mint,
            Self::Goonfi(p) => p.base_mint,
            Self::Solfi(p) => p.base_mint,
        }
    }

    pub fn quote_mint(&self) -> Pubkey {
        match self {
            Self::Humidifi(p) => p.token1_mint,
            Self::Goonfi(p) => p.quote_mint,
            Self::Solfi(p) => p.quote_mint,
        }
    }

    pub fn market(&self) -> Pubkey {
        match self {
            Self::Humidifi(p) => p.market,
            Self::Goonfi(p) => p.market,
            Self::Solfi(p) => p.market,
        }
    }

    pub fn account_pubkeys(&self) -> Vec<Pubkey> {
        match self {
            Self::Humidifi(p) => match p.version {
                SwapVersion::V1 => vec![p.market, p.base_ta, p.quote_ta],
                SwapVersion::V2 | SwapVersion::V3 => {
                    vec![p.market, p.base_ta, p.quote_ta, p.token0_mint, p.token1_mint, p.add1, p.vote]
                }
            },
            Self::Goonfi(p) => vec![p.market, p.base_ta, p.quote_ta, p.blacklist],
            Self::Solfi(p) => {
                vec![p.market, p.base_ta, p.quote_ta, p.cfg, p.oracle, p.base_mint, p.quote_mint]
            }
        }
    }
}

/// Runtime config loaded from `config.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub keypair_path: String,
    /// Which PropAMM to swap through: `humidifi` | `goonfi` | `solfi`.
    pub dex: Dex,
    /// HumidiFi-only: `v1` | `v2` | `v3`. Ignored for goonfi/solfi.
    #[serde(default = "default_humidifi_version")]
    pub version: SwapVersion,
    pub amount_in: u64,
    /// `0` = base→quote, `1` = quote→base.
    pub direction: u8,
    #[serde(deserialize_with = "deser_pubkey")]
    pub pool: Pubkey,
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub base_mint: Option<Pubkey>,
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub quote_mint: Option<Pubkey>,
    /// SolFi-only: market config account (defaults to setup.toml WSOL-USDC cfg).
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub cfg: Option<Pubkey>,
    /// SolFi-only: oracle account (defaults to setup.toml WSOL-USDC oracle).
    #[serde(default, deserialize_with = "deser_opt_pubkey")]
    pub oracle: Option<Pubkey>,
}

fn default_humidifi_version() -> SwapVersion {
    SwapVersion::V3
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
