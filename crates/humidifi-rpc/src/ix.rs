use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};

use crate::config::{PoolAccounts, SwapVersion};

/// HumidiFi program id.
pub const HUMIDIFI_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp");

/// Jito1 validator vote account — used as the readonly `vote` account on live HumidiFi v2/v3 swaps.
pub const JITO1_VOTE: Pubkey = solana_sdk::pubkey!("J1to1yufRnoWn81KYg1XkTWzmKjnYSnmE2VY8DGUJ9Qv");

pub const SWAP_SELECTOR: &[u8] = &[0x04];
pub const SWAPV2_SELECTOR: &[u8] = &[0x0f];
pub const SWAPV3_SELECTOR: &[u8] = &[0x14];

/// Obfuscated HumidiFi instruction data (same encoding as pmm-sim cfg).
pub fn humidifi_instruction_data(selector: &[u8], amount_in: u64, direction: u8) -> Vec<u8> {
    let swap_id: u64 = 1500;
    let mut data: Vec<u8> = Vec::with_capacity(25);
    data.extend_from_slice(&swap_id.to_le_bytes());
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.push(direction);
    data.extend_from_slice(&[0u8; 7]); // padding
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

fn v1_accounts(pool: &PoolAccounts, payer: Pubkey, user_base_ta: Pubkey, user_quote_ta: Pubkey) -> Vec<AccountMeta> {
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
    ]
}

fn v2v3_accounts(pool: &PoolAccounts, payer: Pubkey, user_base_ta: Pubkey, user_quote_ta: Pubkey) -> Vec<AccountMeta> {
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
    ]
}

/// Build a direct HumidiFi swap-v1 instruction.
pub fn build_humidifi_v1_ix(
    pool: &PoolAccounts,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: HUMIDIFI_PROGRAM_ID,
        accounts: v1_accounts(pool, payer, user_base_ta, user_quote_ta),
        data: humidifi_instruction_data(SWAP_SELECTOR, amount_in, direction),
    }
}

/// Build a direct HumidiFi swap-v2 instruction.
pub fn build_humidifi_v2_ix(
    pool: &PoolAccounts,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: HUMIDIFI_PROGRAM_ID,
        accounts: v2v3_accounts(pool, payer, user_base_ta, user_quote_ta),
        data: humidifi_instruction_data(SWAPV2_SELECTOR, amount_in, direction),
    }
}

/// Build a direct HumidiFi swap-v3 instruction.
pub fn build_humidifi_v3_ix(
    pool: &PoolAccounts,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    Instruction {
        program_id: HUMIDIFI_PROGRAM_ID,
        accounts: v2v3_accounts(pool, payer, user_base_ta, user_quote_ta),
        data: humidifi_instruction_data(SWAPV3_SELECTOR, amount_in, direction),
    }
}

/// Dispatch to the matching direct build helper.
pub fn build_humidifi_ix(
    version: SwapVersion,
    pool: &PoolAccounts,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
    amount_in: u64,
    direction: u8,
) -> Instruction {
    match version {
        SwapVersion::V1 => build_humidifi_v1_ix(pool, payer, user_base_ta, user_quote_ta, amount_in, direction),
        SwapVersion::V2 => build_humidifi_v2_ix(pool, payer, user_base_ta, user_quote_ta, amount_in, direction),
        SwapVersion::V3 => build_humidifi_v3_ix(pool, payer, user_base_ta, user_quote_ta, amount_in, direction),
    }
}
