use magnus_shared::pmm_humidifi;
use pmm_sim::cfg::{Cfg, HumidifiSwapV1, HumidifiSwapV2, Swap};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

use crate::config::SwapVersion;

fn humidifi_program_id() -> Pubkey {
    Pubkey::new_from_array(pmm_humidifi::id().to_bytes())
}

/// Build a direct HumidiFi swap-v1 instruction for `market`.
pub fn build_humidifi_v1_ix(
    market: &HumidifiSwapV1,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: humidifi_program_id(),
        accounts: market.swap_accounts(payer, user_base_ta, user_quote_ta, None, None),
        data: market.instruction_data(pmm_humidifi::SWAP_SELECTOR, amount_in, direction),
    }
}

/// Build a direct HumidiFi swap-v2 instruction for `market`.
pub fn build_humidifi_v2_ix(
    market: &HumidifiSwapV2,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: humidifi_program_id(),
        accounts: market.swap_accounts(payer, user_base_ta, user_quote_ta, None, None),
        data: market.instruction_data(pmm_humidifi::SWAPV2_SELECTOR, amount_in, direction),
    }
}

/// Build a direct HumidiFi swap-v3 instruction for `market`.
pub fn build_humidifi_v3_ix(
    market: &HumidifiSwapV2,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: humidifi_program_id(),
        accounts: market.swap_accounts(payer, user_base_ta, user_quote_ta, None, None),
        data: market.instruction_data(pmm_humidifi::SWAPV3_SELECTOR, amount_in, direction),
    }
}

/// Resolve the pool from `setup.toml` and build the matching direct swap ix.
pub fn build_humidifi_ix(
    cfg: &Cfg,
    version: SwapVersion,
    pool: Pubkey,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> eyre::Result<Instruction> {
    let humidifi = cfg.humidifi.as_ref().ok_or_else(|| eyre::eyre!("HumidiFi section missing from setup.toml"))?;

    match version {
        SwapVersion::V1 => {
            let market = humidifi
                .swap_v1
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v1 market {pool} not found in setup.toml"))?;
            Ok(build_humidifi_v1_ix(market, payer, user_base_ta, user_quote_ta, amount_in, direction))
        }
        SwapVersion::V2 => {
            let market = humidifi
                .swap_v2
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v2 market {pool} not found in setup.toml"))?;
            Ok(build_humidifi_v2_ix(market, payer, user_base_ta, user_quote_ta, amount_in, direction))
        }
        SwapVersion::V3 => {
            let market = humidifi
                .swap_v3
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v3 market {pool} not found in setup.toml"))?;
            Ok(build_humidifi_v3_ix(market, payer, user_base_ta, user_quote_ta, amount_in, direction))
        }
    }
}

/// Pool account pubkeys required by the chosen HumidiFi version (from cfg).
pub fn pool_account_pubkeys(cfg: &Cfg, version: SwapVersion, pool: Pubkey) -> eyre::Result<Vec<Pubkey>> {
    let humidifi = cfg.humidifi.as_ref().ok_or_else(|| eyre::eyre!("HumidiFi section missing from setup.toml"))?;

    match version {
        SwapVersion::V1 => {
            let market = humidifi
                .swap_v1
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v1 market {pool} not found in setup.toml"))?;
            Ok(market.accounts())
        }
        SwapVersion::V2 => {
            let market = humidifi
                .swap_v2
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v2 market {pool} not found in setup.toml"))?;
            Ok(market.accounts())
        }
        SwapVersion::V3 => {
            let market = humidifi
                .swap_v3
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v3 market {pool} not found in setup.toml"))?;
            Ok(market.accounts())
        }
    }
}

/// Resolve base/quote mints for ATA derivation.
pub fn resolve_mints(cfg: &Cfg, version: SwapVersion, pool: Pubkey, base_mint_from_rpc: Option<Pubkey>, quote_mint_from_rpc: Option<Pubkey>) -> eyre::Result<(Pubkey, Pubkey)> {
    let humidifi = cfg.humidifi.as_ref().ok_or_else(|| eyre::eyre!("HumidiFi section missing from setup.toml"))?;

    match version {
        SwapVersion::V1 => {
            let base = base_mint_from_rpc.ok_or_else(|| eyre::eyre!("v1 requires base mint from RPC (base_ta)"))?;
            let quote = quote_mint_from_rpc.ok_or_else(|| eyre::eyre!("v1 requires quote mint from RPC (quote_ta)"))?;
            Ok((base, quote))
        }
        SwapVersion::V2 => {
            let market = humidifi
                .swap_v2
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v2 market {pool} not found in setup.toml"))?;
            Ok((market.token0_mint, market.token1_mint))
        }
        SwapVersion::V3 => {
            let market = humidifi
                .swap_v3
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v3 market {pool} not found in setup.toml"))?;
            Ok((market.token0_mint, market.token1_mint))
        }
    }
}
