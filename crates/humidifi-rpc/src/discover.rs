use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcTransactionConfig,
    rpc_request::TokenAccountsFilter,
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::{
    EncodedTransaction, EncodedTransactionWithStatusMeta, UiInstruction, UiMessage,
    UiParsedInstruction, UiTransactionEncoding, UiTransactionStatusMeta,
    option_serializer::OptionSerializer,
};
use spl_token::state::Account as TokenAccount;
use std::str::FromStr;

use crate::config::{PoolAccounts, SwapVersion};
use crate::ix::{HUMIDIFI_PROGRAM_ID, JITO1_VOTE};

#[derive(Debug, Clone)]
struct Vault {
    pubkey: Pubkey,
    mint: Pubkey,
    amount: u64,
}

/// Discover current pool accounts from RPC:
/// 1. vault TAs via `getTokenAccountsByOwner(market)`
/// 2. for v2/v3, `add1` + `vote` from the latest HumidiFi swap ix on this market
pub fn discover_pool_accounts(
    client: &RpcClient,
    market: Pubkey,
    version: SwapVersion,
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
) -> eyre::Result<PoolAccounts> {
    let vaults = fetch_market_vaults(client, market)?;
    println!("discovered {} token vault(s) owned by market {market}", vaults.len());
    for v in &vaults {
        println!("  vault {}  mint={}  amount={}", v.pubkey, v.mint, v.amount);
    }

    let (base_ta, quote_ta, token0_mint, token1_mint) = select_vault_pair(&vaults, base_mint, quote_mint)?;

    let (add1, vote) = match version {
        SwapVersion::V1 => (Pubkey::default(), Pubkey::default()),
        SwapVersion::V2 | SwapVersion::V3 => discover_add1_and_vote(client, market)?,
    };

    let pool = PoolAccounts {
        market,
        base_ta,
        quote_ta,
        token0_mint,
        token1_mint,
        add1,
        vote,
    };

    println!("resolved pool accounts:");
    println!("  market:      {}", pool.market);
    println!("  base_ta:     {} (mint {})", pool.base_ta, pool.token0_mint);
    println!("  quote_ta:    {} (mint {})", pool.quote_ta, pool.token1_mint);
    if matches!(version, SwapVersion::V2 | SwapVersion::V3) {
        println!("  add1:        {}  (from latest live swap; changes over time)", pool.add1);
        println!(
            "  vote:        {}  {}",
            pool.vote,
            if pool.vote == JITO1_VOTE {
                "(Jito1 validator vote account)"
            } else {
                "(non-Jito1 — unusual for this pool)"
            }
        );
    }

    Ok(pool)
}

fn fetch_market_vaults(client: &RpcClient, market: Pubkey) -> eyre::Result<Vec<Vault>> {
    let keyed = client
        .get_token_accounts_by_owner(&market, TokenAccountsFilter::ProgramId(spl_token::id()))
        .map_err(|e| eyre::eyre!("getTokenAccountsByOwner({market}) failed: {e}"))?;

    let pubkeys: Vec<Pubkey> = keyed
        .iter()
        .map(|k| Pubkey::from_str(&k.pubkey).map_err(|e| eyre::eyre!("bad vault pubkey {}: {e}", k.pubkey)))
        .collect::<eyre::Result<_>>()?;

    if pubkeys.is_empty() {
        eyre::bail!("market {market} owns no SPL token accounts");
    }

    let accounts = client
        .get_multiple_accounts(&pubkeys)
        .map_err(|e| eyre::eyre!("failed to fetch vault account data: {e}"))?;

    let mut vaults = Vec::new();
    for (pubkey, acc) in pubkeys.into_iter().zip(accounts) {
        let Some(acc) = acc else { continue };
        let Ok(token) = TokenAccount::unpack(&acc.data) else { continue };
        vaults.push(Vault {
            pubkey,
            mint: token.mint,
            amount: token.amount,
        });
    }
    Ok(vaults)
}

fn select_vault_pair(
    vaults: &[Vault],
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
) -> eyre::Result<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    match (base_mint, quote_mint) {
        (Some(base_mint), Some(quote_mint)) => {
            let base = vaults
                .iter()
                .filter(|v| v.mint == base_mint)
                .max_by_key(|v| v.amount)
                .ok_or_else(|| eyre::eyre!("no vault for base_mint {base_mint}"))?;
            let quote = vaults
                .iter()
                .filter(|v| v.mint == quote_mint)
                .max_by_key(|v| v.amount)
                .ok_or_else(|| eyre::eyre!("no vault for quote_mint {quote_mint}"))?;
            Ok((base.pubkey, quote.pubkey, base_mint, quote_mint))
        }
        (None, None) => {
            let mut sorted = vaults.to_vec();
            sorted.sort_by(|a, b| b.amount.cmp(&a.amount));
            if sorted.len() < 2 {
                eyre::bail!("need at least 2 vaults to infer base/quote; set base_mint/quote_mint in config");
            }
            let a = &sorted[0];
            let b = &sorted[1];
            println!(
                "base_mint/quote_mint not set — using two largest vaults:\n  {} ({})\n  {} ({})",
                a.mint, a.amount, b.mint, b.amount
            );
            Ok((a.pubkey, b.pubkey, a.mint, b.mint))
        }
        _ => eyre::bail!("set both base_mint and quote_mint, or neither"),
    }
}

fn discover_add1_and_vote(client: &RpcClient, market: Pubkey) -> eyre::Result<(Pubkey, Pubkey)> {
    let sigs = client
        .get_signatures_for_address(&market)
        .map_err(|e| eyre::eyre!("getSignaturesForAddress({market}) failed: {e}"))?;

    for status in sigs.iter().take(40) {
        if status.err.is_some() {
            continue;
        }
        let Ok(sig) = Signature::from_str(&status.signature) else { continue };
        let Ok(tx) = client.get_transaction_with_config(
            &sig,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            },
        ) else {
            continue;
        };

        if let Some((add1, vote)) = extract_add1_vote(&tx.transaction, market) {
            println!("add1/vote taken from recent swap sig {}", status.signature);
            return Ok((add1, vote));
        }
    }

    eyre::bail!(
        "could not find a recent HumidiFi v2/v3 swap for market {market} to learn add1/vote. \
         vote is normally Jito1 ({JITO1_VOTE}); add1 changes per swap and must come from a live tx"
    )
}

fn extract_add1_vote(tx: &EncodedTransactionWithStatusMeta, market: Pubkey) -> Option<(Pubkey, Pubkey)> {
    // Prefer scanning account-pubkey lists from partially-decoded HumidiFi ixs (works for CPI inners).
    for accounts in iter_humidifi_account_lists(tx) {
        if accounts.len() >= 14 && accounts[1] == market {
            return Some((accounts[12], accounts[13]));
        }
    }
    None
}

fn iter_humidifi_account_lists(tx: &EncodedTransactionWithStatusMeta) -> Vec<Vec<Pubkey>> {
    let mut out = Vec::new();

    if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
        match &ui_tx.message {
            UiMessage::Raw(raw) => {
                let keys: Vec<Pubkey> = raw.account_keys.iter().filter_map(|k| Pubkey::from_str(k).ok()).collect();
                for ix in &raw.instructions {
                    if keys.get(ix.program_id_index as usize) == Some(&HUMIDIFI_PROGRAM_ID) {
                        let accs = ix
                            .accounts
                            .iter()
                            .filter_map(|i| keys.get(*i as usize).copied())
                            .collect::<Vec<_>>();
                        out.push(accs);
                    }
                }
            }
            UiMessage::Parsed(parsed) => {
                push_from_ui_instructions(&parsed.instructions, &mut out);
            }
        }
    }

    if let Some(meta) = &tx.meta {
        push_from_meta_inner(meta, &mut out);
    }

    out
}

fn push_from_ui_instructions(instructions: &[UiInstruction], out: &mut Vec<Vec<Pubkey>>) {
    for ix in instructions {
        match ix {
            UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) => {
                if Pubkey::from_str(&p.program_id).ok() == Some(HUMIDIFI_PROGRAM_ID) {
                    let accs = p.accounts.iter().filter_map(|a| Pubkey::from_str(a).ok()).collect();
                    out.push(accs);
                }
            }
            UiInstruction::Compiled(c) => {
                // Without a key table we cannot resolve compiled-only inners here.
                let _ = c;
            }
            _ => {}
        }
    }
}

fn push_from_meta_inner(meta: &UiTransactionStatusMeta, out: &mut Vec<Vec<Pubkey>>) {
    let OptionSerializer::Some(inners) = &meta.inner_instructions else {
        return;
    };
    for inner in inners {
        push_from_ui_instructions(&inner.instructions, out);
    }
}
