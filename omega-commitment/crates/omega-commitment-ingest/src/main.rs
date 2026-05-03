//! omega-ingest CLI.
//!
//! Per-sub-tree subcommands that take Cardano source data (CBOR) and
//! emit the JSON format consumed by `omega-commitment commit`.
//!
//! v0.8.0: only `utxo` is fully implemented; the other four are
//! scaffolded and will fail with a clear message pointing to the
//! follow-up plan.

use anyhow::Result;
use clap::{Parser, Subcommand};
use omega_commitment_ingest::{
    governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
    token_policy::ingest_token_policies, utxo::ingest_utxos,
};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-ingest", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Ingest UTXOs from a CBOR snapshot and emit the JSON format
    /// consumed by `omega-commitment commit --sub-tree utxo`.
    Utxo {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: token policy ingestion is not yet implemented.
    TokenPolicy {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: script ingestion is not yet implemented.
    Script {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: stake ingestion is not yet implemented.
    Stake {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: governance ingestion is not yet implemented.
    Governance {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Utxo { input, output } => run_utxo(input, output),
        Cmd::TokenPolicy { input, output } => run_token_policy(input, output),
        Cmd::Script { input, output } => run_script(input, output),
        Cmd::Stake { input, output } => run_stake(input, output),
        Cmd::Governance { input, output } => run_governance(input, output),
    }
}

fn run_utxo(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor =
        fs::read(&input).map_err(|e| anyhow::anyhow!("cannot read {}: {}", input.display(), e))?;
    let out = ingest_utxos(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!(
        "ok: ingested {} utxos -> {}",
        out.utxos.len(),
        output.display()
    );
    Ok(())
}

fn run_token_policy(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let out = ingest_token_policies(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!(
        "ok: ingested {} policies -> {}",
        out.policies.len(),
        output.display()
    );
    Ok(())
}

fn run_script(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let out = ingest_scripts(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!(
        "ok: ingested {} scripts -> {}",
        out.scripts.len(),
        output.display()
    );
    Ok(())
}

fn run_stake(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let out = ingest_stake(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!(
        "ok: ingested {} stake_entries -> {}",
        out.stake_entries.len(),
        output.display()
    );
    Ok(())
}

fn run_governance(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let out = ingest_governance(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!(
        "ok: ingested {} facts -> {}",
        out.facts.len(),
        output.display()
    );
    Ok(())
}
