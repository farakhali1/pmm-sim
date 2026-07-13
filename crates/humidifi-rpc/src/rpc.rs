use pmm_sim::cfg::Cfg;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{account::Account, program_pack::Pack, pubkey::Pubkey};
use spl_token::state::Account as TokenAccount;

use crate::{config::SwapVersion, ix};

#[derive(Debug)]
pub struct FetchedAccounts {
    pub slot: u64,
    pub accounts: Vec<(Pubkey, Option<Account>)>,
}

/// Fetch pool + user accounts in one `getMultipleAccounts` call.
pub fn fetch_swap_accounts(
    client: &RpcClient,
    keys: &[Pubkey],
) -> eyre::Result<FetchedAccounts> {
    let response = client.get_multiple_accounts_with_commitment(keys, CommitmentConfig::confirmed())?;
    let slot = response.context.slot;
    let accounts = keys.iter().copied().zip(response.value.into_iter()).collect();
    Ok(FetchedAccounts { slot, accounts })
}

pub fn print_fetched_accounts(fetched: &FetchedAccounts) {
    println!("fetched {} accounts at slot {}", fetched.accounts.len(), fetched.slot);
    for (pubkey, acc) in &fetched.accounts {
        match acc {
            Some(a) => println!(
                "  ok   {pubkey}  lamports={}  owner={}  data_len={}",
                a.lamports,
                a.owner,
                a.data.len()
            ),
            None => println!("  miss {pubkey}"),
        }
    }
}

/// Read mint pubkey from an SPL token account fetched via RPC.
pub fn mint_from_token_account(client: &RpcClient, token_account: &Pubkey) -> eyre::Result<Pubkey> {
    let acc = client
        .get_account(token_account)
        .map_err(|e| eyre::eyre!("failed to fetch token account {token_account}: {e}"))?;
    let token = TokenAccount::unpack(&acc.data)
        .map_err(|e| eyre::eyre!("failed to unpack token account {token_account}: {e}"))?;
    Ok(token.mint)
}

/// Collect pubkeys needed for the swap (pool cfg accounts + payer + user ATAs).
pub fn collect_keys(
    setup: &Cfg,
    version: SwapVersion,
    pool: Pubkey,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
) -> eyre::Result<Vec<Pubkey>> {
    let mut keys = ix::pool_account_pubkeys(setup, version, pool)?;
    keys.push(payer);
    keys.push(user_base_ta);
    keys.push(user_quote_ta);
    keys.sort();
    keys.dedup();
    Ok(keys)
}
