//! Whole-UTxO snapshot via the Node-to-Client local-state-query miniprotocol.
//!
//! Bypasses `cardano-cli ... query utxo --whole-utxo` (documented as
//! testnet-only and broken on mainnet by the Word16-VLE TxIx decoder bug;
//! see PR IntersectMBO/cardano-cli#1350). Pallas's CBOR decoder doesn't
//! share that asymmetry. Output is the raw `BlockQuery::GetUTxOWhole`
//! response CBOR — the same shape the patched cardano-cli would write
//! with `--output-cbor-bin`.
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

    /// Network: mainnet | preview | preprod | <numeric magic>.
    #[arg(long, default_value = "mainnet")]
    network: String,

    /// Era index to query (Conway = 6, Babbage = 5, Alonzo = 4, ...).
    #[arg(long, default_value_t = ERA_CONWAY)]
    era: u16,

    /// Output file for the raw CBOR response.
    #[arg(long)]
    out: PathBuf,
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

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let args = Args::parse();
    let magic = parse_magic(&args.network)?;
    let started = Instant::now();

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

    sq.acquire(None)
        .await
        .context("acquire latest tip via LocalStateQuery")?;

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

    let bytes = raw.to_vec();
    let len = bytes.len();
    tokio::fs::write(&args.out, &bytes)
        .await
        .with_context(|| format!("write {}", args.out.display()))?;

    sq.send_release().await.context("LSQ release")?;
    sq.send_done().await.context("LSQ done")?;

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
