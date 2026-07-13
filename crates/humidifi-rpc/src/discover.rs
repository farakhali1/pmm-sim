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

use crate::config::{
    Dex, GoonfiPool, HumidifiPool, ResolvedPool, SolfiPool, SwapVersion,
};
use crate::ix::{GOONFI_PROGRAM_ID, HUMIDIFI_PROGRAM_ID, JITO1_VOTE};

#[derive(Debug, Clone)]
struct Vault {
    pubkey: Pubkey,
    mint: Pubkey,
    amount: u64,
}

pub fn discover_pool(
    client: &RpcClient,
    dex: Dex,
    market: Pubkey,
    version: SwapVersion,
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
    solfi_cfg: Option<Pubkey>,
    solfi_oracle: Option<Pubkey>,
) -> eyre::Result<ResolvedPool> {
    match dex {
        Dex::Humidifi => Ok(ResolvedPool::Humidifi(discover_humidifi(
            client, market, version, base_mint, quote_mint,
        )?)),
        Dex::Goonfi => Ok(ResolvedPool::Goonfi(discover_goonfi(
            client, market, base_mint, quote_mint,
        )?)),
        Dex::Solfi => Ok(ResolvedPool::Solfi(discover_solfi(
            client, market, base_mint, quote_mint, solfi_cfg, solfi_oracle,
        )?)),
    }
}

fn discover_humidifi(
    client: &RpcClient,
    market: Pubkey,
    version: SwapVersion,
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
) -> eyre::Result<HumidifiPool> {
    let vaults = fetch_market_vaults(client, market)?;
    print_vaults(market, &vaults);
    let (base_ta, quote_ta, token0_mint, token1_mint) = select_vault_pair(&vaults, base_mint, quote_mint)?;

    let (add1, vote) = match version {
        SwapVersion::V1 => (Pubkey::default(), Pubkey::default()),
        SwapVersion::V2 | SwapVersion::V3 => {
            let accs = find_program_swap_accounts(client, market, HUMIDIFI_PROGRAM_ID, 14)?;
            let add1 = accs[12];
            let vote = accs[13];
            println!("humidifi add1/vote from latest live swap");
            println!(
                "  add1: {} (ephemeral)",
                add1
            );
            println!(
                "  vote: {} {}",
                vote,
                if vote == JITO1_VOTE { "(Jito1)" } else { "" }
            );
            (add1, vote)
        }
    };

    let pool = HumidifiPool {
        market,
        base_ta,
        quote_ta,
        token0_mint,
        token1_mint,
        add1,
        vote,
        version,
    };
    println!(
        "resolved humidifi {}: market={} base_ta={} quote_ta={}",
        version, pool.market, pool.base_ta, pool.quote_ta
    );
    Ok(pool)
}

fn discover_goonfi(
    client: &RpcClient,
    market: Pubkey,
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
) -> eyre::Result<GoonfiPool> {
    // Prefer vaults owned by market; fall back to accounts from a recent swap ix.
    let vaults = fetch_market_vaults(client, market).unwrap_or_default();
    if !vaults.is_empty() {
        print_vaults(market, &vaults);
    }

    let accs = find_program_swap_accounts(client, market, GOONFI_PROGRAM_ID, 9)?;
    // [1]=market [4]=base_ta [5]=quote_ta [6]=blacklist
    let base_ta = accs[4];
    let quote_ta = accs[5];
    let blacklist = accs[6];

    let (resolved_base_mint, resolved_quote_mint) = match (base_mint, quote_mint) {
        (Some(b), Some(q)) => (b, q),
        _ => {
            let b = mint_of(client, &base_ta)?;
            let q = mint_of(client, &quote_ta)?;
            (b, q)
        }
    };

    // If vaults exist and mints were provided, prefer matching vault pubkeys by mint.
    let (base_ta, quote_ta) = if let (Some(b), Some(q)) = (base_mint, quote_mint) {
        if let Ok((bt, qt, _, _)) = select_vault_pair(&vaults, Some(b), Some(q)) {
            (bt, qt)
        } else {
            (base_ta, quote_ta)
        }
    } else {
        (base_ta, quote_ta)
    };

    let pool = GoonfiPool {
        market,
        base_ta,
        quote_ta,
        blacklist,
        base_mint: resolved_base_mint,
        quote_mint: resolved_quote_mint,
    };
    println!("resolved goonfi:");
    println!("  market:    {}", pool.market);
    println!("  base_ta:   {} (mint {})", pool.base_ta, pool.base_mint);
    println!("  quote_ta:  {} (mint {})", pool.quote_ta, pool.quote_mint);
    println!("  blacklist: {}", pool.blacklist);
    Ok(pool)
}

/// Default SolFi cfg/oracle for the WSOL-USDC market in cfg/setup.toml.
const DEFAULT_SOLFI_CFG: Pubkey = solana_sdk::pubkey!("FmxXDSR9WvpJTCh738D1LEDuhMoA8geCtZgHb3isy7Dp");
const DEFAULT_SOLFI_ORACLE: Pubkey = solana_sdk::pubkey!("2ny7eGyZCoeEVTkNLf5HcnJFBKkyA4p4gcrtb3b8y8ou");

fn discover_solfi(
    client: &RpcClient,
    market: Pubkey,
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
    solfi_cfg: Option<Pubkey>,
    solfi_oracle: Option<Pubkey>,
) -> eyre::Result<SolfiPool> {
    let vaults = fetch_market_vaults(client, market)?;
    print_vaults(market, &vaults);

    let cfg = solfi_cfg.unwrap_or(DEFAULT_SOLFI_CFG);
    let oracle = solfi_oracle.unwrap_or(DEFAULT_SOLFI_ORACLE);

    let (base_ta, quote_ta, base_mint, quote_mint) = select_vault_pair(&vaults, base_mint, quote_mint)?;

    let pool = SolfiPool {
        market,
        base_ta,
        quote_ta,
        cfg,
        oracle,
        base_mint,
        quote_mint,
    };
    println!("resolved solfi:");
    println!("  market:  {}", pool.market);
    println!("  base_ta: {} (mint {})", pool.base_ta, pool.base_mint);
    println!("  quote_ta:{} (mint {})", pool.quote_ta, pool.quote_mint);
    println!("  cfg:     {}", pool.cfg);
    println!("  oracle:  {}", pool.oracle);
    Ok(pool)
}

fn print_vaults(market: Pubkey, vaults: &[Vault]) {
    println!("discovered {} token vault(s) owned by market {market}", vaults.len());
    for v in vaults {
        println!("  vault {}  mint={}  amount={}", v.pubkey, v.mint, v.amount);
    }
}

fn mint_of(client: &RpcClient, token_account: &Pubkey) -> eyre::Result<Pubkey> {
    let acc = client
        .get_account(token_account)
        .map_err(|e| eyre::eyre!("failed to fetch token account {token_account}: {e}"))?;
    let token = TokenAccount::unpack(&acc.data)
        .map_err(|e| eyre::eyre!("failed to unpack token account {token_account}: {e}"))?;
    Ok(token.mint)
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
        return Ok(vec![]);
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
                eyre::bail!("need at least 2 vaults; set base_mint/quote_mint in config");
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

/// Find a recent program swap ix for `market` and return its account list.
///
/// Prefer successful txs, but also accept failed ones — account metas (oracle/cfg/etc.)
/// are still present and are what we need for discovery.
fn find_program_swap_accounts(
    client: &RpcClient,
    market: Pubkey,
    program_id: Pubkey,
    min_accounts: usize,
) -> eyre::Result<Vec<Pubkey>> {
    let sigs = client
        .get_signatures_for_address(&market)
        .map_err(|e| eyre::eyre!("getSignaturesForAddress({market}) failed: {e}"))?;

    // Pass 1: successful txs. Pass 2: failed txs (still usable for account discovery).
    for require_ok in [true, false] {
        for status in sigs.iter().take(40) {
            if require_ok && status.err.is_some() {
                continue;
            }
            if !require_ok && status.err.is_none() {
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

            for accounts in iter_program_account_lists(&tx.transaction, program_id) {
                if accounts.len() >= min_accounts && accounts.get(1) == Some(&market) {
                    println!(
                        "accounts taken from recent swap sig {} ({})",
                        status.signature,
                        if status.err.is_none() { "ok" } else { "failed-tx" }
                    );
                    return Ok(accounts);
                }
            }
        }
    }

    eyre::bail!("could not find a recent {program_id} swap for market {market}")
}

fn iter_program_account_lists(tx: &EncodedTransactionWithStatusMeta, program_id: Pubkey) -> Vec<Vec<Pubkey>> {
    let mut out = Vec::new();

    if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
        match &ui_tx.message {
            UiMessage::Raw(raw) => {
                let keys: Vec<Pubkey> = raw.account_keys.iter().filter_map(|k| Pubkey::from_str(k).ok()).collect();
                for ix in &raw.instructions {
                    if keys.get(ix.program_id_index as usize) == Some(&program_id) {
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
                push_from_ui_instructions(&parsed.instructions, program_id, &mut out);
            }
        }
    }

    if let Some(meta) = &tx.meta {
        push_from_meta_inner(meta, program_id, &mut out);
    }

    out
}

fn push_from_ui_instructions(instructions: &[UiInstruction], program_id: Pubkey, out: &mut Vec<Vec<Pubkey>>) {
    for ix in instructions {
        if let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) = ix {
            if Pubkey::from_str(&p.program_id).ok() == Some(program_id) {
                let accs = p.accounts.iter().filter_map(|a| Pubkey::from_str(a).ok()).collect();
                out.push(accs);
            }
        }
    }
}

fn push_from_meta_inner(meta: &UiTransactionStatusMeta, program_id: Pubkey, out: &mut Vec<Vec<Pubkey>>) {
    let OptionSerializer::Some(inners) = &meta.inner_instructions else {
        return;
    };
    for inner in inners {
        push_from_ui_instructions(&inner.instructions, program_id, out);
    }
}
