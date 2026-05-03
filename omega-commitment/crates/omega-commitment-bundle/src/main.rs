//! omega-bundle CLI.
//!
//! Two subcommands:
//!   - `assemble` reads seven sub-tree inputs and emits `bundle.json`.
//!   - `verify` re-runs assembly against the same inputs and confirms
//!     the bundle's roots match.

use clap::{Parser, Subcommand};
use omega_commitment_bundle::bundle::{assemble, verify, BundleRecord};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-bundle", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read seven sub-tree inputs from --input-dir and emit bundle.json.
    Assemble {
        /// Directory containing the seven sub-tree input JSON files
        /// (utxo.json, header.json, tx_index.json, token_policy.json,
        /// script.json, stake.json, governance.json).
        #[arg(short, long)]
        input_dir: PathBuf,
        /// Output path for bundle.json.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Re-run assembly against --input-dir and confirm the bundle's
    /// roots match. Exits non-zero on mismatch.
    Verify {
        /// Path to a previously-assembled bundle.json.
        #[arg(short, long)]
        bundle: PathBuf,
        /// Directory containing the seven sub-tree input JSON files.
        #[arg(short, long)]
        input_dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Assemble { input_dir, output } => cmd_assemble(input_dir, output),
        Cmd::Verify { bundle, input_dir } => cmd_verify(bundle, input_dir),
    }
}

fn cmd_assemble(input_dir: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let input_dir = input_dir
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve input-dir {}: {}", input_dir.display(), e))?;
    let bundle = assemble(&input_dir)?;
    fs::write(&output, serde_json::to_string_pretty(&bundle)?)?;
    println!(
        "ok: assembled bundle blake2b={} sha3={}",
        hex::encode(bundle.blake2b_bundle_root),
        hex::encode(bundle.sha3_bundle_root)
    );
    Ok(())
}

fn cmd_verify(bundle_path: PathBuf, input_dir: PathBuf) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&bundle_path)
        .map_err(|e| anyhow::anyhow!("cannot read bundle {}: {}", bundle_path.display(), e))?;
    let bundle: BundleRecord = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("cannot parse bundle {}: {}", bundle_path.display(), e))?;
    let input_dir = input_dir
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve input-dir {}: {}", input_dir.display(), e))?;
    verify(&bundle, &input_dir)?;
    println!(
        "ok: bundle verifies blake2b={} sha3={}",
        hex::encode(bundle.blake2b_bundle_root),
        hex::encode(bundle.sha3_bundle_root)
    );
    Ok(())
}
