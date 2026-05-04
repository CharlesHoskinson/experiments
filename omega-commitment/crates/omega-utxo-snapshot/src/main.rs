//! Whole-UTxO snapshot via the Node-to-Client local-state-query miniprotocol.
//!
//! Bypasses `cardano-cli ... query utxo --whole-utxo` (documented as
//! testnet-only and broken on mainnet by the Word16-VLE TxIx decoder bug;
//! see PR IntersectMBO/cardano-cli#1350). Pallas's CBOR decoder doesn't
//! share that asymmetry. Output is the raw `BlockQuery::GetUTxOWhole`
//! response CBOR — the same shape the patched cardano-cli would write
//! with `--output-cbor-bin`.
//!
//! ## Acquisition modes
//!
//! - `--manifest <path>`: production / reproducible-snapshot mode. The
//!   JSON manifest pins `(block_hash, slot, epoch, stability_depth,
//!   stake_snapshot_select)`. The binary calls
//!   `acquire(Some(Point::Specific(slot, hash)))` so the LSQ session is
//!   anchored to that exact point. Stability depth must be ≥ k = 2160
//!   (enforced by `SnapshotManifest::validate`).
//!
//! - `--snapshot-tip`: experimental smoke-test mode. The binary calls
//!   `acquire(None)` and snapshots whatever the node currently treats
//!   as tip. Prints a stderr warning so operators don't accidentally
//!   ship a wandering-tip snapshot to consumers.
//!
//! - Neither flag: defaults to `--snapshot-tip` for back-compat with
//!   v0.9.x experiments. Same warning is printed.
//!
//! Memory profile: pallas-network 0.30's `Client::query<_, AnyCbor>`
//! buffers the entire response in memory before returning, so the binary
//! holds the full UTxO CBOR (~multi-GB on mainnet) as a `Vec<u8>` before
//! the single `tokio::fs::write`. Linear RSS growth during the query is
//! expected. A true streaming path would require dropping into the
//! lower-level pallas multiplexer and incrementally parsing the LSQ
//! message stream — left for a follow-up if memory becomes the bottleneck.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use omega_commitment_core::snapshot_manifest::SnapshotManifest;
use pallas_codec::utils::AnyCbor;
use pallas_network::facades::NodeClient;
use pallas_network::miniprotocols::localstate::queries_v16::{BlockQuery, LedgerQuery, Request};
use pallas_network::miniprotocols::{Point, MAINNET_MAGIC, PREPROD_MAGIC, PREVIEW_MAGIC};

const ERA_CONWAY: u16 = 6;

#[derive(Parser, Debug)]
#[command(version, about = "Dump the whole UTxO set via N2C local-state-query")]
struct Args {
    /// Path to the cardano-node Unix socket.
    #[arg(long)]
    socket: PathBuf,

    /// Network: mainnet | preview | preprod | `<numeric magic>`.
    #[arg(long, default_value = "mainnet")]
    network: String,

    /// Era index to query (Conway = 6, Babbage = 5, Alonzo = 4, ...).
    #[arg(long, default_value_t = ERA_CONWAY)]
    era: u16,

    /// Output file for the raw CBOR response.
    #[arg(long)]
    out: PathBuf,

    /// Path to a `SnapshotManifest` JSON pinning the chain point. When
    /// supplied, acquisition uses `Point::Specific(slot, block_hash)`
    /// for reproducibility. Mutually exclusive with `--snapshot-tip`.
    #[arg(long, conflicts_with = "snapshot_tip")]
    manifest: Option<PathBuf>,

    /// Snapshot the wandering tip via `acquire(None)`. Experimental
    /// smoke-test mode only; production runs MUST use `--manifest`.
    #[arg(long, conflicts_with = "manifest")]
    snapshot_tip: bool,
}

fn parse_magic(s: &str) -> Result<u64> {
    Ok(match s {
        "mainnet" => MAINNET_MAGIC,
        "preview" => PREVIEW_MAGIC,
        "preprod" => PREPROD_MAGIC,
        other => other.parse::<u64>().with_context(|| {
            format!("network must be mainnet|preview|preprod|<u64>, got {other:?}")
        })?,
    })
}

/// Decide which acquisition target the LSQ session should pin.
///
/// Returns `Some(Point::Specific(slot, hash))` if `--manifest` was
/// supplied; otherwise returns `None` (which `LocalStateQuery::acquire`
/// interprets as "snapshot the wandering tip"). Emits a stderr warning
/// in the wandering-tip case.
fn resolve_target(
    manifest_path: Option<&std::path::Path>,
    snapshot_tip_flag: bool,
) -> Result<Option<Point>> {
    match manifest_path {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("read manifest {}", path.display()))?;
            let manifest: SnapshotManifest = serde_json::from_str(&raw)
                .with_context(|| format!("parse manifest {}", path.display()))?;
            manifest
                .validate()
                .with_context(|| format!("validate manifest {}", path.display()))?;
            eprintln!(
                "omega-utxo-snapshot: pinning chain point slot={} block_hash={} (epoch={}, stability_depth={})",
                manifest.slot,
                hex::encode(manifest.block_hash),
                manifest.epoch,
                manifest.stability_depth
            );
            Ok(Some(Point::Specific(
                manifest.slot,
                manifest.block_hash.to_vec(),
            )))
        }
        None => {
            if snapshot_tip_flag {
                eprintln!(
                    "omega-utxo-snapshot: WARNING --snapshot-tip selected; snapshotting the wandering tip via acquire(None). EXPERIMENTAL smoke-test mode — production runs must use --manifest <path>."
                );
            } else {
                eprintln!(
                    "omega-utxo-snapshot: WARNING neither --manifest nor --snapshot-tip supplied; defaulting to wandering-tip mode for back-compat with v0.9.x experiments. EXPERIMENTAL smoke-test mode — production runs must use --manifest <path>."
                );
            }
            Ok(None)
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let args = Args::parse();
    let magic = parse_magic(&args.network)?;
    let started = Instant::now();

    let target = resolve_target(args.manifest.as_deref(), args.snapshot_tip)?;

    eprintln!(
        "omega-utxo-snapshot: connecting to {} (magic={}, era={})",
        args.socket.display(),
        magic,
        args.era
    );

    let mut client = NodeClient::connect(&args.socket, magic)
        .await
        .with_context(|| format!("connect+handshake against {}", args.socket.display()))?;

    let sq = client.statequery();

    sq.acquire(target)
        .await
        .context("acquire chain point via LocalStateQuery")?;

    let tip: Point = pallas_network::miniprotocols::localstate::queries_v16::get_chain_point(sq)
        .await
        .context("get_chain_point after acquire")?;
    let (slot, hash) = match &tip {
        Point::Origin => (0, Vec::new()),
        Point::Specific(s, h) => (*s, h.clone()),
    };
    eprintln!(
        "omega-utxo-snapshot: acquired tip slot={} hash={}",
        slot,
        hex::encode(&hash)
    );

    let request = Request::LedgerQuery(LedgerQuery::BlockQuery(args.era, BlockQuery::GetUTxOWhole));
    eprintln!(
        "omega-utxo-snapshot: issuing GetUTxOWhole (this may take minutes; ~10M UTxOs on mainnet)"
    );

    let raw: AnyCbor = sq
        .query(request)
        .await
        .context("GetUTxOWhole query failed")?;

    // Release the LSQ session BEFORE the multi-GB disk write so the
    // node-side acquisition lifetime is bounded by the network round-trip
    // rather than by local disk I/O. Closes audit finding A6/F001
    // (Batch 5, 2026-05-03 resolution plan).
    sq.send_release().await.context("LSQ release")?;
    sq.send_done().await.context("LSQ done")?;

    // Move bytes out of `AnyCbor` without copying: pallas-codec 0.30.2
    // exposes `AnyCbor::unwrap(self) -> Vec<u8>` which surrenders the
    // inner `Vec<u8>` allocation directly. The previous `raw.to_vec()`
    // call cloned the buffer (multi-GB on mainnet); `unwrap()` is a
    // zero-copy move. Closes audit finding A6/F001.
    let bytes = raw.unwrap();
    let len = bytes.len();
    tokio::fs::write(&args.out, &bytes)
        .await
        .with_context(|| format!("write {}", args.out.display()))?;

    // TODO(v1.0 Task 4): implement pallas_codec::minicbor::Decoder<'_, GetUTxOWholeResponse>
    // pass to validate the bytes the LSQ producer emits before they reach the
    // omega-ingest mainnet parser. Audit finding A2/F001 (deferred per the
    // 2026-05-03 audit resolution plan, Batch 4 step 6).

    let elapsed = started.elapsed();
    eprintln!(
        "omega-utxo-snapshot: wrote {} bytes ({:.2} MiB) to {} in {:.1?} (tip slot={})",
        len,
        len as f64 / (1024.0 * 1024.0),
        args.out.display(),
        elapsed,
        slot
    );

    Ok(())
}
