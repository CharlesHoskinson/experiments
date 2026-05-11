//! `omega-toy-consensus run` - boot a single LoganNet node from CLI flags.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use omega_toy_consensus::{NodeConfig, PeerConfig, RpcConfig};

#[derive(Parser, Debug)]
#[command(name = "omega-toy-consensus")]
#[command(about = "Local LoganNet 3-node Raft cluster harness", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Boots a single Raft node and serves JSON-RPC.
    Run(RunArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// Stable u64 node identifier (must be non-zero).
    #[arg(long)]
    node_id: u64,
    /// Path to the SQLite WAL directory; created if absent.
    #[arg(long)]
    data_dir: PathBuf,
    /// Path to the libp2p identity keypair file; created if absent.
    #[arg(long)]
    identity_file: Option<PathBuf>,
    /// Libp2p multiaddr the node listens on.
    #[arg(long)]
    listen: String,
    /// Static peer; format: `<id>,<peer_id>,<libp2p_addr>,<rpc_url>`.
    /// Repeat once per peer.
    #[arg(long = "peer", value_name = "ID,PEER_ID,ADDR,URL")]
    peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind address.
    #[arg(long)]
    rpc: std::net::SocketAddr,
    /// Cluster identifier; must match across peers.
    #[arg(long, default_value = "loganet-dev")]
    cluster_id: String,
    /// Apply deadline in seconds. Bounds the JSON-RPC `omega_submitClaim`
    /// server path (raft commit + apply).
    #[arg(long, default_value = "5")]
    apply_deadline_secs: u64,
    /// Per-peer libp2p raft RPC timeout in seconds. When unset, defaults
    /// to `--apply_deadline_secs`. Set explicitly when peer-to-peer RPCs
    /// need a different bound than the client-write deadline.
    #[arg(long)]
    raft_rpc_timeout_secs: Option<u64>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run(args) => run(args).await,
    }
}

async fn run(args: RunArgs) -> anyhow::Result<()> {
    if args.node_id == 0 {
        anyhow::bail!("--node_id must be non-zero (openraft requires non-zero NodeId)");
    }
    let config = NodeConfig {
        node_id: args.node_id,
        data_dir: args.data_dir,
        identity_file: args.identity_file,
        libp2p_listen: args.listen,
        peers: args.peers,
        rpc: RpcConfig {
            bind: args.rpc,
            max_batch: 25,
            max_request_bytes: 1024 * 1024,
        },
        cluster_id: args.cluster_id,
        apply_deadline: Duration::from_secs(args.apply_deadline_secs),
        raft_rpc_timeout: args.raft_rpc_timeout_secs.map(Duration::from_secs),
    };

    tracing::info!(node_id = config.node_id, rpc = %config.rpc.bind, "starting");

    let handle = omega_toy_consensus::start(config).await?;

    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown requested");
    handle.shutdown().await?;
    Ok(())
}
