//! omega-commitment CLI.
//!
//! Subcommand `commit`: read a JSON sub-tree input, emit:
//!   - a `commitment.json` containing the root + metadata + input digest
//!   - a `witnesses/<leaf_hash>.json` per leaf

use clap::{Parser, Subcommand, ValueEnum};
use omega_commitment_core::{
    governance_state_leaf::GovernanceFact,
    hash::{blake3_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-commitment", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build a sub-tree commitment from a JSON input.
    Commit {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short = 's', long, value_enum, default_value_t = SubTree::Utxo)]
        sub_tree: SubTree,
        /// Maximum input file size in bytes. Default 2 GiB.
        #[arg(long, default_value_t = 2_147_483_648u64)]
        max_input_bytes: u64,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
enum SubTree {
    Utxo,
    Header,
    TxIndex,
    TokenPolicy,
    Script,
    Stake,
    Governance,
}

#[derive(Deserialize)]
struct UtxoInput {
    utxos: Vec<Utxo>,
}

#[derive(Deserialize)]
struct HeaderInput {
    headers: Vec<BlockHeader>,
}

#[derive(Deserialize)]
struct TxIndexInput {
    entries: Vec<TxIndexEntry>,
}

#[derive(Deserialize)]
struct TokenPolicyInput {
    policies: Vec<TokenPolicy>,
}

#[derive(Deserialize)]
struct ScriptInput {
    scripts: Vec<ScriptEntry>,
}

#[derive(Deserialize)]
struct StakeInput {
    stake_entries: Vec<StakeEntry>,
}

#[derive(Deserialize)]
struct GovernanceInput {
    facts: Vec<GovernanceFact>,
}

#[derive(Serialize)]
struct CommitmentRecord {
    /// Which Ω-Commitment sub-tree this record describes.
    sub_tree: SubTree,
    /// Blake3-256 of the raw input file bytes. Lets consumers confirm
    /// the commitment is bound to a specific input snapshot.
    #[serde(with = "hex::serde")]
    input_digest: Hash,
    /// The Merkle root of this sub-tree.
    #[serde(with = "hex::serde")]
    root: Hash,
    /// Number of leaves in the tree AFTER padding to the next power of two.
    /// Always >= `item_count`. Equal when `item_count` is itself a power
    /// of two.
    leaf_count: usize,
    /// Depth of the tree, where depth 0 means a single-leaf tree (root == leaf).
    tree_depth: usize,
    /// Number of items in the input BEFORE padding. The semantic unit
    /// depends on `sub_tree`: UTXOs, headers, or tx-index entries.
    item_count: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Commit {
            input,
            output,
            sub_tree,
            max_input_bytes,
        } => commit(input, output, sub_tree, max_input_bytes),
    }
}

fn safe_child(base_dir: &std::path::Path, child: &std::path::Path) -> anyhow::Result<PathBuf> {
    let parent = child
        .parent()
        .ok_or_else(|| anyhow::anyhow!("write path has no parent: {}", child.display()))?;
    let parent = parent
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve write parent {}: {}", parent.display(), e))?;
    let file_name = child
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("write path has no filename: {}", child.display()))?;
    let resolved = parent.join(file_name);
    if !resolved.starts_with(base_dir) {
        anyhow::bail!(
            "write path escapes output dir: {} not in {}",
            resolved.display(),
            base_dir.display()
        );
    }
    Ok(resolved)
}

fn build_utxo_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: UtxoInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed
        .utxos
        .iter()
        .map(|u| u.leaf_hash())
        .collect::<Result<Vec<_>, _>>()?;
    let n = parsed.utxos.len();
    Ok((leaves, n))
}

fn build_header_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: HeaderInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.headers.iter().map(|h| h.leaf_hash()).collect();
    let n = parsed.headers.len();
    Ok((leaves, n))
}

fn build_tx_index_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: TxIndexInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.entries.iter().map(|e| e.leaf_hash()).collect();
    let n = parsed.entries.len();
    Ok((leaves, n))
}

fn build_token_policy_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: TokenPolicyInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.policies.iter().map(|p| p.leaf_hash()).collect();
    let n = parsed.policies.len();
    Ok((leaves, n))
}

fn build_script_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: ScriptInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.scripts.iter().map(|s| s.leaf_hash()).collect();
    let n = parsed.scripts.len();
    Ok((leaves, n))
}

fn build_stake_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: StakeInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let n = parsed.stake_entries.len();
    Ok((leaves, n))
}

fn build_governance_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: GovernanceInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.facts.iter().map(|f| f.leaf_hash()).collect();
    let n = parsed.facts.len();
    Ok((leaves, n))
}

fn commit(
    input: PathBuf,
    output: PathBuf,
    sub_tree: SubTree,
    max_input_bytes: u64,
) -> anyhow::Result<()> {
    let input = input
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot read input {}: {}", input.display(), e))?;
    let metadata = fs::metadata(&input)
        .map_err(|e| anyhow::anyhow!("cannot stat input {}: {}", input.display(), e))?;
    if metadata.len() > max_input_bytes {
        anyhow::bail!(
            "input file {} is {} bytes, exceeds --max-input-bytes={}",
            input.display(),
            metadata.len(),
            max_input_bytes
        );
    }
    fs::create_dir_all(&output)
        .map_err(|e| anyhow::anyhow!("cannot create output dir {}: {}", output.display(), e))?;
    let output = output
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve output {}: {}", output.display(), e))?;

    let raw = fs::read_to_string(&input)?;
    let input_digest = blake3_256(raw.as_bytes());

    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => build_utxo_leaves(&raw)?,
        SubTree::Header => build_header_leaves(&raw)?,
        SubTree::TxIndex => build_tx_index_leaves(&raw)?,
        SubTree::TokenPolicy => build_token_policy_leaves(&raw)?,
        SubTree::Script => build_script_leaves(&raw)?,
        SubTree::Stake => build_stake_leaves(&raw)?,
        SubTree::Governance => build_governance_leaves(&raw)?,
    };

    let tree = MerkleTree::build(leaves.clone());

    let witness_dir = output.join("witnesses");
    fs::create_dir_all(&witness_dir)?;

    let record = CommitmentRecord {
        sub_tree,
        input_digest,
        root: tree.root(),
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        item_count,
    };
    {
        let mut tmp = tempfile::Builder::new()
            .prefix(".commitment.")
            .suffix(".json.tmp")
            .tempfile_in(&output)?;
        use std::io::Write;
        tmp.write_all(serde_json::to_string_pretty(&record)?.as_bytes())?;
        tmp.flush()?;
        tmp.persist(output.join("commitment.json"))?;
    }

    let mut leaf_idx: HashMap<Hash, u32> = HashMap::with_capacity(tree.leaf_count());
    for (i, h) in tree.leaves().iter().enumerate() {
        leaf_idx.entry(*h).or_insert(i as u32);
    }
    for leaf in leaves {
        let idx = *leaf_idx
            .get(&leaf)
            .ok_or_else(|| anyhow::anyhow!("leaf vanished from tree"))?;
        let w = InclusionWitness::build_at_index(&tree, idx)
            .ok_or_else(|| anyhow::anyhow!("index out of range"))?;
        let fname = format!("{}.json", hex::encode(leaf));
        let target = safe_child(&output, &witness_dir.join(&fname))?;
        fs::write(&target, serde_json::to_string_pretty(&w)?)?;
    }

    println!(
        "ok: sub_tree={:?} root={} input_digest={} items={}",
        record.sub_tree,
        hex::encode(record.root),
        hex::encode(record.input_digest),
        record.item_count
    );
    Ok(())
}
