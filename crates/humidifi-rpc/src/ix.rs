use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};

use crate::config::{GoonfiPool, HumidifiPool, ResolvedPool, SolfiPool, SwapVersion};

pub const HUMIDIFI_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp");
pub const GOONFI_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("goonERTdGsjnkZqWuVjs73BZ3Pb9qoCUdBUL17BnS5j");
pub const SOLFI_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("SV2EYYJyRz2YhfXwXnhNAevDEui5Q6yrfyo13WtupPF");

/// Jito1 validator vote account — used as the readonly `vote` account on live HumidiFi v2/v3 swaps.
pub const JITO1_VOTE: Pubkey = solana_sdk::pubkey!("J1to1yufRnoWn81KYg1XkTWzmKjnYSnmE2VY8DGUJ9Qv");

pub const HUMIDIFI_SWAP_SELECTOR: &[u8] = &[0x04];
pub const HUMIDIFI_SWAPV2_SELECTOR: &[u8] = &[0x0f];
pub const HUMIDIFI_SWAPV3_SELECTOR: &[u8] = &[0x14];
pub const GOONFI_SWAP_SELECTOR: &[u8] = &[0x02];
pub const SOLFI_SWAP_SELECTOR: &[u8] = &[0x07];

pub fn humidifi_instruction_data(selector: &[u8], amount_in: u64, direction: u8) -> Vec<u8> {
    let swap_id: u64 = 1500;
    let mut data: Vec<u8> = Vec::with_capacity(25);
    data.extend_from_slice(&swap_id.to_le_bytes());
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.push(direction);
    data.extend_from_slice(&[0u8; 7]);
    data.extend_from_slice(selector);

    let key: u64 = u64::from_le_bytes([58, 255, 47, 255, 226, 186, 235, 195]);
    for (i, chunk) in data.chunks_exact_mut(8).enumerate() {
        let qword = u64::from_le_bytes(chunk.try_into().unwrap());
        let pos_mask = (0x0001_0001_0001_0001u64).wrapping_mul(i as u64);
        let obfuscated = qword ^ key ^ pos_mask;
        chunk.copy_from_slice(&obfuscated.to_le_bytes());
    }
    let remainder_start = data.len() / 8 * 8;
    if remainder_start < data.len() {
        let pos_mask = (0x0001_0001_0001_0001u64).wrapping_mul((remainder_start / 8) as u64);
        let mut rem = [0u8; 8];
        let rem_len = data.len() - remainder_start;
        rem[..rem_len].copy_from_slice(&data[remainder_start..]);
        let qword = u64::from_le_bytes(rem);
        let obfuscated = qword ^ key ^ pos_mask;
        let ob_bytes = obfuscated.to_le_bytes();
        data[remainder_start..].copy_from_slice(&ob_bytes[..rem_len]);
    }
    data
}

pub fn goonfi_instruction_data(amount_in: u64, is_bid: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(19);
    data.extend_from_slice(GOONFI_SWAP_SELECTOR);
    data.extend_from_slice(&[is_bid, 0u8]); // is_bid + bump
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data
}

pub fn solfi_instruction_data(amount_in: u64, direction: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(18);
    data.extend_from_slice(SOLFI_SWAP_SELECTOR);
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.push(direction);
    data
}

pub fn build_humidifi_ix(
    pool: &HumidifiPool,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    let (accounts, selector) = match pool.version {
        SwapVersion::V1 => (
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(pool.market, false),
                AccountMeta::new(pool.base_ta, false),
                AccountMeta::new(pool.quote_ta, false),
                AccountMeta::new(user_base_ta, false),
                AccountMeta::new(user_quote_ta, false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(sysvar::instructions::id(), false),
            ],
            HUMIDIFI_SWAP_SELECTOR,
        ),
        SwapVersion::V2 | SwapVersion::V3 => {
            let selector = if pool.version == SwapVersion::V2 {
                HUMIDIFI_SWAPV2_SELECTOR
            } else {
                HUMIDIFI_SWAPV3_SELECTOR
            };
            (
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(pool.market, false),
                    AccountMeta::new(pool.base_ta, false),
                    AccountMeta::new(pool.quote_ta, false),
                    AccountMeta::new(user_base_ta, false),
                    AccountMeta::new(user_quote_ta, false),
                    AccountMeta::new_readonly(sysvar::clock::id(), false),
                    AccountMeta::new_readonly(spl_token::id(), false),
                    AccountMeta::new_readonly(spl_token::id(), false),
                    AccountMeta::new_readonly(sysvar::instructions::id(), false),
                    AccountMeta::new(pool.token0_mint, false),
                    AccountMeta::new(pool.token1_mint, false),
                    AccountMeta::new(pool.add1, false),
                    AccountMeta::new_readonly(pool.vote, false),
                ],
                selector,
            )
        }
    };

    Instruction {
        program_id: HUMIDIFI_PROGRAM_ID,
        accounts,
        data: humidifi_instruction_data(selector, amount_in, direction),
    }
}

/// Direct GoonFi swap. `direction`: 0 = sell base (not bid), 1 = buy base with quote (is_bid).
pub fn build_goonfi_ix(
    pool: &GoonfiPool,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: GOONFI_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pool.market, false),
            AccountMeta::new(user_base_ta, false),
            AccountMeta::new(user_quote_ta, false),
            AccountMeta::new(pool.base_ta, false),
            AccountMeta::new(pool.quote_ta, false),
            AccountMeta::new_readonly(pool.blacklist, false),
            AccountMeta::new_readonly(sysvar::instructions::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: goonfi_instruction_data(amount_in, direction),
    }
}

/// Direct SolFi v2 swap. `direction`: 0 = base→quote, 1 = quote→base.
pub fn build_solfi_ix(
    pool: &SolfiPool,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: SOLFI_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pool.market, false),
            AccountMeta::new_readonly(pool.oracle, false),
            AccountMeta::new_readonly(pool.cfg, false),
            AccountMeta::new(pool.base_ta, false),
            AccountMeta::new(pool.quote_ta, false),
            AccountMeta::new(user_base_ta, false),
            AccountMeta::new(user_quote_ta, false),
            AccountMeta::new(pool.base_mint, false),
            AccountMeta::new(pool.quote_mint, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(sysvar::instructions::id(), false),
        ],
        data: solfi_instruction_data(amount_in, direction),
    }
}

pub fn build_swap_ix(
    pool: &ResolvedPool,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    match pool {
        ResolvedPool::Humidifi(p) => build_humidifi_ix(p, payer, user_base_ta, user_quote_ta, amount_in, direction),
        ResolvedPool::Goonfi(p) => build_goonfi_ix(p, payer, user_base_ta, user_quote_ta, amount_in, direction),
        ResolvedPool::Solfi(p) => build_solfi_ix(p, payer, user_base_ta, user_quote_ta, amount_in, direction),
    }
}
