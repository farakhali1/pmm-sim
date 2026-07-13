mod config;
mod ix;
mod rpc;

use std::path::PathBuf;

use clap::Parser;
use config::{Config, SwapVersion};
use pmm_sim::cfg::Cfg;
use solana_client::{rpc_client::RpcClient, rpc_config::RpcSimulateTransactionConfig};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, read_keypair_file},
    signer::Signer,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

#[derive(Debug, Parser)]
#[command(name = "humidifi-rpc", about = "Build & simulate direct HumidiFi v1/v2/v3 swaps over RPC")]
struct Cli {
    /// Path to config.json
    #[arg(long, default_value = "config.json")]
    config: PathBuf,
}

fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load(&cli.config)?;
    let pool = cfg.pool_pubkey()?;
    let payer = load_keypair(&cfg.keypair_path)?;
    let payer_pubkey = payer.pubkey();

    println!("config: {}", cli.config.display());
    println!("rpc: {}", cfg.rpc_url);
    println!("payer: {payer_pubkey}");
    println!("pool: {pool}");
    println!("version: {}", cfg.version);
    println!("amount_in: {}", cfg.amount_in);
    println!(
        "direction: {} ({})",
        cfg.direction,
        if cfg.direction == 0 { "base→quote" } else { "quote→base" }
    );

    let setup = Cfg::load(&cfg.setup_path)?;
    let client = RpcClient::new_with_commitment(cfg.rpc_url.clone(), CommitmentConfig::confirmed());

    let (base_mint, quote_mint) = resolve_base_quote_mints(&client, &setup, cfg.version, pool)?;
    let user_base_ta = get_associated_token_address(&payer_pubkey, &base_mint);
    let user_quote_ta = get_associated_token_address(&payer_pubkey, &quote_mint);

    println!("base_mint:  {base_mint}");
    println!("quote_mint: {quote_mint}");
    println!("user_base_ta:  {user_base_ta}");
    println!("user_quote_ta: {user_quote_ta}");

    let keys = rpc::collect_keys(&setup, cfg.version, pool, payer_pubkey, user_base_ta, user_quote_ta)?;
    let fetched = rpc::fetch_swap_accounts(&client, &keys)?;
    rpc::print_fetched_accounts(&fetched);

    let ix = ix::build_humidifi_ix(
        &setup,
        cfg.version,
        pool,
        payer_pubkey,
        user_base_ta,
        user_quote_ta,
        cfg.amount_in,
        cfg.direction,
    )?;

    println!(
        "built {} ix: program={} accounts={} data_len={}",
        cfg.version,
        ix.program_id,
        ix.accounts.len(),
        ix.data.len()
    );
    for (i, meta) in ix.accounts.iter().enumerate() {
        println!(
            "  [{i:02}] {} writable={} signer={}",
            meta.pubkey, meta.is_writable, meta.is_signer
        );
    }

    let blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pubkey), &[&payer], blockhash);

    let sim = client.simulate_transaction_with_config(
        &tx,
        RpcSimulateTransactionConfig {
            sig_verify: true,
            commitment: Some(CommitmentConfig::confirmed()),
            ..RpcSimulateTransactionConfig::default()
        },
    )?;

    let value = sim.value;
    println!("--- simulation (slot context {}) ---", sim.context.slot);
    println!("err: {:?}", value.err);
    println!("units_consumed: {:?}", value.units_consumed);
    if let Some(logs) = value.logs {
        println!("logs:");
        for line in logs {
            println!("  {line}");
        }
    }

    if value.err.is_some() {
        eyre::bail!("simulation failed");
    }

    Ok(())
}

fn load_keypair(path: &str) -> eyre::Result<Keypair> {
    read_keypair_file(path).map_err(|e| eyre::eyre!("failed to read keypair from {path}: {e}"))
}

fn resolve_base_quote_mints(
    client: &RpcClient,
    setup: &Cfg,
    version: SwapVersion,
    pool: Pubkey,
) -> eyre::Result<(Pubkey, Pubkey)> {
    match version {
        SwapVersion::V1 => {
            let humidifi = setup.humidifi.as_ref().ok_or_else(|| eyre::eyre!("HumidiFi missing from setup.toml"))?;
            let market = humidifi
                .swap_v1
                .get(&pool)
                .ok_or_else(|| eyre::eyre!("HumidiFi v1 market {pool} not found in setup.toml"))?;
            let base_mint = rpc::mint_from_token_account(client, &market.base_ta)?;
            let quote_mint = rpc::mint_from_token_account(client, &market.quote_ta)?;
            Ok((base_mint, quote_mint))
        }
        SwapVersion::V2 | SwapVersion::V3 => ix::resolve_mints(setup, version, pool, None, None),
    }
}
