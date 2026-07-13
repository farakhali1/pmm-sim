use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::config::ResolvedPool;

#[derive(Debug)]
pub struct FetchedAccounts {
    pub slot: u64,
    pub accounts: Vec<(Pubkey, Option<Account>)>,
}

pub fn fetch_swap_accounts(client: &RpcClient, keys: &[Pubkey]) -> eyre::Result<FetchedAccounts> {
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

pub fn collect_keys(
    pool: &ResolvedPool,
    payer: Pubkey,
    user_base_ta: Pubkey,
    user_quote_ta: Pubkey,
) -> Vec<Pubkey> {
    let mut keys = pool.account_pubkeys();
    keys.push(payer);
    keys.push(user_base_ta);
    keys.push(user_quote_ta);
    keys.sort();
    keys.dedup();
    keys
}
