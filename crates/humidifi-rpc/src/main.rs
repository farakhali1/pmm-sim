mod config;
mod discover;
mod ix;
mod rpc;

use std::path::PathBuf;

use clap::Parser;
use config::Config;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    signature::{Keypair, read_keypair_file},
    signer::Signer,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

#[derive(Debug, Parser)]
#[command(
    name = "humidifi-rpc",
    about = "Build & simulate direct PropAMM swaps (HumidiFi / GoonFi / SolFi) over RPC"
)]
struct Cli {
    /// Path to config.json
    #[arg(long, default_value = "config.json")]
    config: PathBuf,
}

fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load(&cli.config)?;
    let payer = load_keypair(&cfg.keypair_path)?;
    let payer_pubkey = payer.pubkey();

    println!("config: {}", cli.config.display());
    println!("rpc: {}", cfg.rpc_url);
    println!("payer: {payer_pubkey}");
    println!("dex: {}", cfg.dex);
    println!("pool: {}", cfg.pool);
    if matches!(cfg.dex, config::Dex::Humidifi) {
        println!("humidifi version: {}", cfg.version);
    }
    println!("amount_in: {}", cfg.amount_in);
    println!(
        "direction: {} ({})",
        cfg.direction,
        if cfg.direction == 0 { "base→quote" } else { "quote→base" }
    );

    let client = RpcClient::new_with_commitment(cfg.rpc_url.clone(), CommitmentConfig::confirmed());

    let pool = discover::discover_pool(
        &client,
        cfg.dex,
        cfg.pool,
        cfg.version,
        cfg.base_mint,
        cfg.quote_mint,
        cfg.cfg,
        cfg.oracle,
    )?;

    let user_base_ta = get_associated_token_address(&payer_pubkey, &pool.base_mint());
    let user_quote_ta = get_associated_token_address(&payer_pubkey, &pool.quote_mint());
    println!("base_mint:  {}", pool.base_mint());
    println!("quote_mint: {}", pool.quote_mint());
    println!("user_base_ta:  {user_base_ta}");
    println!("user_quote_ta: {user_quote_ta}");

    let keys = rpc::collect_keys(&pool, payer_pubkey, user_base_ta, user_quote_ta);
    let fetched = rpc::fetch_swap_accounts(&client, &keys)?;
    rpc::print_fetched_accounts(&fetched);

    let ix = ix::build_swap_ix(&pool, payer_pubkey, user_base_ta, user_quote_ta, cfg.amount_in, cfg.direction);

    println!(
        "built {} ix: program={} accounts={} data_len={}",
        cfg.dex,
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

    let sim = client.simulate_transaction(&tx)?;
    let value = sim.value;
    println!("--- simulation (slot context {}) ---", sim.context.slot);
    println!("err: {:?}", value.err);
        if value.err.is_none(){
        let resp = client.send_and_confirm_transaction(
            &tx,
        )?;
        println!("tx sent: {resp}");
    }
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
