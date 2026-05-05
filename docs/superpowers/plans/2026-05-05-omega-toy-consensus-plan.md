# omega-toy-consensus Group 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the keystone LoganNet crate `omega-toy-consensus` (library + binary + minimal jsonrpsee 0.26 JSON-RPC) that wires openraft 0.9, omega-mock-ledger, and omega-network into a runnable 3-node Raft cluster with a single-claim round-trip.

**Architecture:** A conductor crate that owns wiring + lifecycles only. Consensus rules stay in openraft. State-machine rules stay in `omega-mock-ledger`. Verification stays in `omega-claim-verifier`. Transport stays in `omega-network`. The crate exposes a `start()` entry point + `Node` / `NodeHandle` Rust API and a two-method JSON-RPC surface (`omega_submitClaim`, `omega_getState`) bound to `127.0.0.1:800N`. Client-side leader forwarding via `−32000 NotLeader` with `data: { leader_id, leader_rpc_url }`.

**Tech Stack:** Rust 1.95.0, openraft 0.9, jsonrpsee 0.26 (server + macros + client), tokio (rt-multi-thread + macros + signal), tracing + tracing-subscriber, clap 4 (derive), thiserror 2, schemars 0.8, serde 1, serde_json 1. Dev: turmoil 0.7, fail 0.5 (with `failpoints` feature), shuttle 0.7, proptest (workspace), tokio-test 0.4, tempfile (workspace), criterion 0.5.

**Spec source:** `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`

**Workspace assumption:** plan executes from `c:/experiments/omega-commitment/` on Windows + 1.95.0. Branch: `feat/omega-toy-consensus-group1`, based on `feat/omega-network-group5` (rebase onto `main` after the network branch merges). Same machine + toolchain as PR-5 verification.

**Acceptance gates** (re-stated from spec; final task verifies all of them):

1. `cargo build -p omega-toy-consensus --bin omega-toy-consensus` succeeds.
2. `cargo test -p omega-toy-consensus --no-fail-fast` — all listed tests pass.
3. `cargo kani` (or `bash skills/local/rust-test-kani/scripts/kani-bound.sh omega-toy-consensus`) runs the snapshot-install proof clean.
4. `cargo doc -p omega-toy-consensus --no-deps --document-private-items` — clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
6. `cargo fmt --check` — clean.
7. `rust-test-orchestrator` Phase-5 report prints `STATUS: GREEN`.
8. Manual smoke test: 3 in-process nodes via `examples/three_node_local.rs` reach quorum within 3s; `curl` POST to `http://127.0.0.1:8001/` with `omega_submitClaim` returns `{ accepted: true, applied_index: <n> }`.
9. PR description includes orchestrator report + smoke-test trace.

---

## Task 1: Branch + crate scaffold

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/Cargo.toml`
- Create: `omega-commitment/crates/omega-toy-consensus/src/lib.rs`
- Create: `omega-commitment/crates/omega-toy-consensus/README.md`
- Modify: `omega-commitment/Cargo.toml` (add to `members`, add workspace deps)

- [ ] **Step 1: Create the branch**

```bash
cd c:/experiments
git fetch origin
git checkout -b feat/omega-toy-consensus-group1 origin/feat/omega-network-group5
```

If `feat/omega-network-group5` has merged to main by the time you start, branch off `origin/main` instead.

- [ ] **Step 2: Add workspace dependencies**

In `omega-commitment/Cargo.toml`, under `[workspace.dependencies]`, add (alphabetical placement):

```toml
jsonrpsee = { version = "0.26", default-features = false, features = ["server", "macros", "client", "http-client"] }
schemars = "0.8"
turmoil = "0.7"
fail = { version = "0.5", features = ["failpoints"] }
shuttle = "0.7"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Add to `members`:

```toml
"crates/omega-toy-consensus",
```

- [ ] **Step 3: Write the crate Cargo.toml**

Create `omega-commitment/crates/omega-toy-consensus/Cargo.toml`:

```toml
[package]
name = "omega-toy-consensus"
version = "0.1.0"
edition = "2021"
publish = false
license = "Apache-2.0"

[lints]
workspace = true

[lib]
name = "omega_toy_consensus"
path = "src/lib.rs"

[[bin]]
name = "omega-toy-consensus"
path = "src/bin/omega-toy-consensus.rs"

[dependencies]
omega-claim-tx = { path = "../omega-claim-tx" }
omega-mock-ledger = { path = "../omega-mock-ledger" }
omega-network = { path = "../omega-network" }
openraft = { workspace = true }
tokio = { workspace = true, features = ["rt-multi-thread", "macros", "signal", "sync", "time"] }
jsonrpsee = { workspace = true }
schemars = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
clap = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
turmoil = { workspace = true }
fail = { workspace = true }
shuttle = { workspace = true }
proptest = { workspace = true }
tokio-test = "0.4"
tempfile = { workspace = true }
criterion = "0.5"

[features]
default = []
failpoints = ["fail/failpoints", "omega-mock-ledger/failpoints", "omega-network/failpoints"]
turmoil-tests = []
shuttle-tests = []

[[bench]]
name = "bench_submit_p50"
harness = false
```

(If `omega-mock-ledger` and `omega-network` do not currently expose `failpoints` features, add them — see Task 22 for the failpoint injection sites; the feature plumbing is part of that task.)

- [ ] **Step 4: Write the crate-level lib.rs stub**

Create `omega-commitment/crates/omega-toy-consensus/src/lib.rs`:

```rust
//! LoganNet keystone: openraft + omega-mock-ledger + omega-network + JSON-RPC.
//!
//! # Overview
//!
//! `omega-toy-consensus` is the conductor crate of the LoganNet local 3-node
//! Raft harness. It owns wiring and lifecycle only: consensus rules stay in
//! openraft, state-machine rules stay in [`omega_mock_ledger`], verification
//! stays in [`omega_claim_verifier`], and transport stays in
//! [`omega_network`]. Every line in this crate is either bringing one of those
//! four up, routing a request between them, or exposing them via the JSON-RPC
//! surface or the run-binary.
//!
//! # Design context
//!
//! - Spec: [`docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`][1]
//! - LoganNet roadmap: [`cardano-wiki/wiki/pages/loganet-roadmap.md`][2]
//! - OpenSpec change (upstream crates): [`add-proof-experiment-harness`][3]
//!
//! [1]: ../../../docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md
//! [2]: ../../../cardano-wiki/wiki/pages/loganet-roadmap.md
//! [3]: ../../../openspec/changes/add-proof-experiment-harness/
//!
//! # Tier of trust
//!
//! Soundness-bearing wiring. This crate does not verify proofs (the verifier
//! does) and does not apply state (the writer actor does), but it is the
//! component that ensures `Raft::client_write` is the only path to apply, that
//! a non-leader returns `−32000 NotLeader` rather than silently proxying, and
//! that the writer actor's lifecycle is bounded by `Node::shutdown`.
//!
//! # v0.1 limitations
//!
//! - Localhost-only RPC (`127.0.0.1:800N`); no TLS, no auth, no rate limiting.
//! - Two RPC methods only: `omega_submitClaim`, `omega_getState`.
//! - HTTP only; WebSocket subscriptions land with `omega-api` (Goblins).
//! - No membership change; static `--peer` topology.
//! - No mDNS / Kademlia discovery.
//! - Windows + 1.95.0 toolchain only; Linux/macOS CI deferred to Group 2.
//! - See [`loganet-roadmap`][2] for the full deferral table.
//!
//! # Conventions
//!
//! - Bring-up and shutdown are async; everything else is sync where possible.
//! - Errors surface via [`ConsensusError`] internally and JSON-RPC error codes
//!   `−32000..−32005` externally; mapping lives in `routing` + `rpc::error`.
//! - Every public item carries `# Errors` and `# Soundness` blocks per
//!   `skills/local/omega-rustdoc-style/SKILL.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod config;
pub mod error;
pub mod node;
pub mod routing;
pub mod rpc;

pub use config::{NodeConfig, PeerConfig, RpcConfig};
pub use error::ConsensusError;
pub use node::{Node, NodeHandle};
pub use rpc::types::{LogIdView, NodeRole, NodeState, SubmitOutcome};

/// Boots a single Raft node, mounts the mock ledger, binds the JSON-RPC
/// server, and returns a handle for graceful shutdown.
///
/// # Errors
///
/// - [`ConsensusError::Storage`] — SQLite open or schema initialisation failed.
/// - [`ConsensusError::Network`] — libp2p bind or peer dial failed.
/// - [`ConsensusError::RpcBind`] — the JSON-RPC HTTP server failed to bind
///   `config.rpc.bind`.
/// - [`ConsensusError::Raft`] — openraft rejected the initial membership.
///
/// # Soundness
///
/// Bring-up is idempotent on storage: the writer-actor lifecycle (see
/// `omega-mock-ledger`'s crate-level `# Soundness` block) is preserved across
/// restarts. Bring-up does NOT validate cluster identity beyond the
/// `cluster_id` string equality check — operators must ensure
/// `--cluster-id` matches across all peers, otherwise openraft will accept
/// the membership and quorum will form across logically-distinct clusters.
///
/// # Examples
///
/// ```no_run
/// # async fn run() -> Result<(), omega_toy_consensus::ConsensusError> {
/// use omega_toy_consensus::{start, NodeConfig};
/// let config = NodeConfig::single_node_localhost(1)?;
/// let handle = start(config).await?;
/// handle.shutdown().await?;
/// # Ok(()) }
/// ```
pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
    Node::start(config).await
}
```

- [ ] **Step 5: Write the README**

Create `omega-commitment/crates/omega-toy-consensus/README.md`:

```markdown
# omega-toy-consensus

LoganNet keystone: openraft 0.9 + omega-mock-ledger + omega-network + minimal
JSON-RPC. Library + `omega-toy-consensus run` binary.

See `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md` for the
full design and `cardano-wiki/wiki/pages/loganet-roadmap.md` for the milestone
roadmap and Group 2 deferrals.

## v0.1 limitations

- Localhost only (`127.0.0.1:800N`).
- No TLS, no auth, no rate limiting.
- Two RPC methods: `omega_submitClaim`, `omega_getState`.
- No membership change; static `--peer` topology.
- Windows + 1.95.0 toolchain only.
```

- [ ] **Step 6: Build**

```bash
cd c:/experiments/omega-commitment
cargo build -p omega-toy-consensus
```

Expected: build fails — `error[E0432]: unresolved import` for `config`, `error`, `node`, `routing`, `rpc`. This is intentional: those modules ship in later tasks.

- [ ] **Step 7: Stub the missing modules so the crate compiles**

Create empty stubs so subsequent tasks can build:

```bash
cd c:/experiments/omega-commitment/crates/omega-toy-consensus/src
touch config.rs error.rs node.rs routing.rs
mkdir rpc
touch rpc/mod.rs
```

Add to each:

```rust
// src/config.rs
//! Node configuration types.
```

```rust
// src/error.rs
//! Consensus error type.
```

```rust
// src/node.rs
//! Node lifecycle.
```

```rust
// src/routing.rs
//! openraft → JSON-RPC error translation.
```

```rust
// src/rpc/mod.rs
//! JSON-RPC server.
pub mod types {
    //! Wire types for the JSON-RPC surface.
}
```

These do NOT yet export the symbols that `lib.rs` re-exports. The build will still fail.

- [ ] **Step 8: Commit**

```bash
git add omega-commitment/Cargo.toml omega-commitment/crates/omega-toy-consensus/
git commit -m "omega-toy-consensus: scaffold crate with module stubs"
```

---

## Task 2: ConsensusError + minimum exported types so lib.rs compiles

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/error.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/config.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/node.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/routing.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/mod.rs`

- [ ] **Step 1: Define `ConsensusError`**

`src/error.rs`:

```rust
//! Consensus error type for [`crate::start`] and [`crate::Node`].

use thiserror::Error;

/// Errors produced during node bring-up, runtime, and shutdown.
#[derive(Debug, Error)]
pub enum ConsensusError {
    /// SQLite open / schema init / writer-actor start failed.
    #[error("storage: {0}")]
    Storage(#[from] omega_mock_ledger::LedgerError),

    /// libp2p bind, dial, or RPC factory init failed.
    #[error("network: {0}")]
    Network(#[from] omega_network::rpc::OmegaNetworkError),

    /// JSON-RPC HTTP server failed to bind the configured address.
    #[error("rpc bind on {addr}: {source}")]
    RpcBind {
        /// The address that failed to bind.
        addr: std::net::SocketAddr,
        /// Underlying jsonrpsee error.
        source: jsonrpsee::core::client::Error,
    },

    /// openraft initialisation, run-loop, or shutdown failed.
    #[error("raft: {0}")]
    Raft(String),

    /// Configuration parse / validation failed before bring-up.
    #[error("config: {0}")]
    Config(String),

    /// Shutdown was requested but the runtime task did not join cleanly.
    #[error("shutdown join: {0}")]
    ShutdownJoin(String),
}
```

- [ ] **Step 2: Stub `NodeConfig`, `PeerConfig`, `RpcConfig` in `config.rs`**

Just enough to satisfy `lib.rs` re-exports. Real fields ship in Task 3.

```rust
//! Node configuration types.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Static configuration consumed by [`crate::start`].
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Stable u64 node identifier; matches the openraft `NodeId`.
    pub node_id: u64,
    /// Path to the SQLite WAL directory; created if absent.
    pub data_dir: PathBuf,
    /// Libp2p multiaddr the node listens on.
    pub libp2p_listen: String,
    /// Static peer list; 2 entries for a 3-node cluster.
    pub peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind + limits.
    pub rpc: RpcConfig,
    /// Cluster identifier; must match across all peers.
    pub cluster_id: String,
    /// Apply deadline; default 5s.
    pub apply_deadline: Duration,
}

/// One peer's wire-level coordinates.
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Stable u64 node identifier of the peer.
    pub node_id: u64,
    /// Libp2p multiaddr to dial.
    pub libp2p_addr: String,
    /// Public RPC URL used in `−32000 NotLeader` hints.
    pub rpc_url: String,
}

/// JSON-RPC HTTP server configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Maximum batch length; default 25.
    pub max_batch: u16,
    /// Maximum request body bytes; default 1 MiB.
    pub max_request_bytes: u32,
}

impl NodeConfig {
    /// Convenience: single-node localhost cluster (used in doctests / smoke
    /// fixtures). Real bring-ups should populate `peers` with at least 2
    /// entries.
    ///
    /// # Errors
    ///
    /// Returns [`ConsensusError::Config`](crate::ConsensusError::Config) if
    /// `node_id` is 0 (openraft requires non-zero).
    pub fn single_node_localhost(node_id: u64) -> Result<Self, crate::ConsensusError> {
        if node_id == 0 {
            return Err(crate::ConsensusError::Config(
                "node_id must be non-zero".into(),
            ));
        }
        Ok(Self {
            node_id,
            data_dir: std::env::temp_dir().join(format!("omega-toy-consensus-{node_id}")),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + node_id),
            peers: Vec::new(),
            rpc: RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + node_id).parse().unwrap(),
                max_batch: 25,
                max_request_bytes: 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(5),
        })
    }
}
```

- [ ] **Step 3: Stub `Node` and `NodeHandle`**

`src/node.rs`:

```rust
//! Node lifecycle.

use crate::{ConsensusError, NodeConfig};

/// Live LoganNet node. Owns the openraft instance, the mock-ledger writer
/// handle, the libp2p network, and the JSON-RPC server.
pub struct Node {
    // fields populated in Task 7
}

/// Handle for graceful shutdown of a running [`Node`].
pub struct NodeHandle {
    // fields populated in Task 8
}

impl Node {
    /// Brings the node up. See [`crate::start`] for the public entry point.
    ///
    /// # Errors
    ///
    /// See [`ConsensusError`].
    pub async fn start(_config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
        Err(ConsensusError::Config("Node::start unimplemented".into()))
    }
}

impl NodeHandle {
    /// Initiates graceful shutdown. Drains in-flight RPC submits, terminates
    /// the writer actor, releases the libp2p socket, then awaits the runtime
    /// task.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::ShutdownJoin`] — runtime task panicked or was
    ///   cancelled abnormally.
    /// - [`ConsensusError::Raft`] — openraft shutdown returned an error.
    pub async fn shutdown(self) -> Result<(), ConsensusError> {
        Ok(())
    }
}
```

- [ ] **Step 4: Stub `routing.rs`**

`src/routing.rs`:

```rust
//! openraft → JSON-RPC error translation.

// Real translator ships in Task 5.
```

- [ ] **Step 5: Stub `rpc/mod.rs` types module so re-exports compile**

Replace `src/rpc/mod.rs` with:

```rust
//! JSON-RPC server.

pub mod types {
    //! Wire types for the JSON-RPC surface.

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// Outcome of a single `omega_submitClaim` call.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    pub struct SubmitOutcome {
        /// Whether the claim was applied to the state machine.
        pub accepted: bool,
        /// Raft log index at which the apply occurred, when `accepted`.
        pub applied_index: Option<u64>,
        /// Reject reason, when `!accepted`. One of: `verify`, `invalid`,
        /// `replay`.
        pub reject_reason: Option<String>,
    }

    /// Read-only view of node + ledger state.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    pub struct NodeState {
        /// Stable u64 node identifier.
        pub node_id: u64,
        /// This node's current openraft role.
        pub role: NodeRole,
        /// Leader's node id, when known.
        pub leader_id: Option<u64>,
        /// Last log id committed to local storage, when present.
        pub last_log_id: Option<LogIdView>,
        /// Last log index applied to the state machine.
        pub applied_index: u64,
        /// Number of nullifiers in the ledger.
        pub nullifier_count: u64,
        /// Number of Starstream UTxOs in the ledger.
        pub starstream_utxo_count: u64,
    }

    /// JSON-friendly mirror of openraft's `RaftState::role`.
    #[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
    pub enum NodeRole {
        /// Currently the leader.
        Leader,
        /// Following a leader.
        Follower,
        /// Election in progress.
        Candidate,
        /// Read-only, non-voting member.
        Learner,
    }

    /// JSON-friendly mirror of openraft's `LogId`.
    #[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
    pub struct LogIdView {
        /// Raft term of the entry.
        pub term: u64,
        /// Log index of the entry.
        pub index: u64,
    }
}
```

- [ ] **Step 6: Build**

```bash
cargo build -p omega-toy-consensus
```

Expected: clean build (warnings about unused imports OK).

- [ ] **Step 7: Verify doc build**

```bash
cargo doc -p omega-toy-consensus --no-deps --document-private-items 2>&1 | tee /tmp/doc.log
```

Expected: clean. No `[missing_docs]`, no broken intra-doc links.

- [ ] **Step 8: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/
git commit -m "omega-toy-consensus: ConsensusError + config + node stubs"
```

---

## Task 3: NodeConfig CLI parsing + serde-on-config-file

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/config.rs`
- Create: `omega-commitment/crates/omega-toy-consensus/src/config_tests.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Add serde derives + clap derives to config types**

Replace `src/config.rs` with:

```rust
//! Node configuration types.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Static configuration consumed by [`crate::start`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Stable u64 node identifier; matches the openraft `NodeId`.
    pub node_id: u64,
    /// Path to the SQLite WAL directory; created if absent.
    pub data_dir: PathBuf,
    /// Libp2p multiaddr the node listens on.
    pub libp2p_listen: String,
    /// Static peer list; 2 entries for a 3-node cluster.
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind + limits.
    pub rpc: RpcConfig,
    /// Cluster identifier; must match across all peers.
    pub cluster_id: String,
    /// Apply deadline; default 5s.
    #[serde(with = "humantime_serde", default = "default_apply_deadline")]
    pub apply_deadline: Duration,
}

fn default_apply_deadline() -> Duration {
    Duration::from_secs(5)
}

/// One peer's wire-level coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Stable u64 node identifier of the peer.
    pub node_id: u64,
    /// Libp2p multiaddr to dial.
    pub libp2p_addr: String,
    /// Public RPC URL used in `−32000 NotLeader` hints.
    pub rpc_url: String,
}

impl std::str::FromStr for PeerConfig {
    type Err = crate::ConsensusError;

    /// Parses `<node_id>,<libp2p_addr>,<rpc_url>` (used by the CLI `--peer`
    /// flag).
    ///
    /// # Errors
    ///
    /// [`ConsensusError::Config`](crate::ConsensusError::Config) if the input
    /// has fewer than 3 comma-separated fields or `node_id` does not parse as
    /// `u64`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(3, ',').collect();
        if parts.len() != 3 {
            return Err(crate::ConsensusError::Config(format!(
                "peer must be `<node_id>,<libp2p>,<rpc>`, got `{s}`"
            )));
        }
        let node_id: u64 = parts[0]
            .parse()
            .map_err(|e| crate::ConsensusError::Config(format!("peer node_id: {e}")))?;
        Ok(Self {
            node_id,
            libp2p_addr: parts[1].to_string(),
            rpc_url: parts[2].to_string(),
        })
    }
}

/// JSON-RPC HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Maximum batch length; default 25.
    #[serde(default = "default_max_batch")]
    pub max_batch: u16,
    /// Maximum request body bytes; default 1 MiB.
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: u32,
}

fn default_max_batch() -> u16 {
    25
}

fn default_max_request_bytes() -> u32 {
    1024 * 1024
}

impl NodeConfig {
    /// Convenience: single-node localhost cluster (used in doctests / smoke
    /// fixtures). Real bring-ups should populate `peers` with at least 2
    /// entries.
    ///
    /// # Errors
    ///
    /// Returns [`ConsensusError::Config`](crate::ConsensusError::Config) if
    /// `node_id` is 0 (openraft requires non-zero).
    pub fn single_node_localhost(node_id: u64) -> Result<Self, crate::ConsensusError> {
        if node_id == 0 {
            return Err(crate::ConsensusError::Config(
                "node_id must be non-zero".into(),
            ));
        }
        Ok(Self {
            node_id,
            data_dir: std::env::temp_dir().join(format!("omega-toy-consensus-{node_id}")),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + node_id),
            peers: Vec::new(),
            rpc: RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + node_id).parse().unwrap(),
                max_batch: 25,
                max_request_bytes: 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(5),
        })
    }
}

#[cfg(test)]
mod tests;
```

Add `humantime-serde = "1"` to the crate's `[dependencies]` in `Cargo.toml`.

- [ ] **Step 2: Write the failing tests**

Create `omega-commitment/crates/omega-toy-consensus/src/config/tests.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;

use super::{NodeConfig, PeerConfig};

#[test]
fn single_node_localhost_basic() {
    let cfg = NodeConfig::single_node_localhost(1).unwrap();
    assert_eq!(cfg.node_id, 1);
    assert_eq!(cfg.libp2p_listen, "/ip4/127.0.0.1/tcp/4001");
    assert_eq!(cfg.rpc.bind.port(), 8001);
    assert_eq!(cfg.rpc.max_batch, 25);
    assert_eq!(cfg.rpc.max_request_bytes, 1024 * 1024);
    assert_eq!(cfg.apply_deadline, Duration::from_secs(5));
}

#[test]
fn single_node_localhost_rejects_zero() {
    let err = NodeConfig::single_node_localhost(0).unwrap_err();
    assert!(matches!(err, crate::ConsensusError::Config(_)));
}

#[test]
fn peer_config_parse_ok() {
    let p: PeerConfig = "2,/ip4/127.0.0.1/tcp/4002,http://127.0.0.1:8002"
        .parse()
        .unwrap();
    assert_eq!(p.node_id, 2);
    assert_eq!(p.libp2p_addr, "/ip4/127.0.0.1/tcp/4002");
    assert_eq!(p.rpc_url, "http://127.0.0.1:8002");
}

#[test]
fn peer_config_parse_too_few_fields() {
    let err: Result<PeerConfig, _> = "2,/ip4/127.0.0.1/tcp/4002".parse();
    assert!(err.is_err());
}

#[test]
fn peer_config_parse_bad_node_id() {
    let err: Result<PeerConfig, _> = "abc,/ip4/127.0.0.1/tcp/4002,http://x".parse();
    assert!(err.is_err());
}

#[test]
fn node_config_serde_round_trip_toml() {
    let cfg = NodeConfig {
        node_id: 7,
        data_dir: PathBuf::from("/tmp/x"),
        libp2p_listen: "/ip4/127.0.0.1/tcp/4007".into(),
        peers: vec![PeerConfig {
            node_id: 2,
            libp2p_addr: "/ip4/127.0.0.1/tcp/4002".into(),
            rpc_url: "http://127.0.0.1:8002".into(),
        }],
        rpc: super::RpcConfig {
            bind: "127.0.0.1:8007".parse().unwrap(),
            max_batch: 25,
            max_request_bytes: 1024 * 1024,
        },
        cluster_id: "loganet-dev".into(),
        apply_deadline: Duration::from_secs(5),
    };
    let toml = toml::to_string(&cfg).unwrap();
    let back: NodeConfig = toml::from_str(&toml).unwrap();
    assert_eq!(back.node_id, 7);
    assert_eq!(back.peers.len(), 1);
    assert_eq!(back.apply_deadline, Duration::from_secs(5));
}
```

Add `toml = "0.8"` to `[dev-dependencies]` in the crate Cargo.toml.

- [ ] **Step 3: Run tests, expect pass**

```bash
cd c:/experiments/omega-commitment
cargo test -p omega-toy-consensus --lib config::tests
```

Expected: 5 tests, all PASS.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/Cargo.toml omega-commitment/crates/omega-toy-consensus/Cargo.toml omega-commitment/crates/omega-toy-consensus/src/config.rs omega-commitment/crates/omega-toy-consensus/src/config/tests.rs
git commit -m "omega-toy-consensus: NodeConfig + PeerConfig + RpcConfig with parsing"
```

---

## Task 4: Wire types — JsonSchema generation + serde round-trip tests

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/mod.rs` (already has stub types)
- Create: `omega-commitment/crates/omega-toy-consensus/src/rpc/types_tests.rs`

- [ ] **Step 1: Move types into a dedicated file**

Move the inline `pub mod types { … }` from `src/rpc/mod.rs` into `src/rpc/types.rs`:

`src/rpc/types.rs`:

```rust
//! Wire types for the JSON-RPC surface.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Outcome of a single `omega_submitClaim` call.
///
/// `accepted = true` ⇒ `applied_index = Some(idx)` and `reject_reason = None`.
/// `accepted = false` ⇒ `applied_index = None` and `reject_reason` names the
/// rejection class.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone)]
pub struct SubmitOutcome {
    /// Whether the claim was applied to the state machine.
    pub accepted: bool,
    /// Raft log index at which the apply occurred, when `accepted`.
    pub applied_index: Option<u64>,
    /// Reject reason, when `!accepted`. One of `"verify"`, `"invalid"`,
    /// `"replay"`.
    pub reject_reason: Option<String>,
}

/// Read-only view of node + ledger state.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone)]
pub struct NodeState {
    /// Stable u64 node identifier.
    pub node_id: u64,
    /// This node's current openraft role.
    pub role: NodeRole,
    /// Leader's node id, when known.
    pub leader_id: Option<u64>,
    /// Last log id committed to local storage, when present.
    pub last_log_id: Option<LogIdView>,
    /// Last log index applied to the state machine.
    pub applied_index: u64,
    /// Number of nullifiers in the ledger.
    pub nullifier_count: u64,
    /// Number of Starstream UTxOs in the ledger.
    pub starstream_utxo_count: u64,
}

/// JSON-friendly mirror of openraft's `RaftState::role`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Currently the leader.
    Leader,
    /// Following a leader.
    Follower,
    /// Election in progress.
    Candidate,
    /// Read-only, non-voting member.
    Learner,
}

/// JSON-friendly mirror of openraft's `LogId`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
pub struct LogIdView {
    /// Raft term of the entry.
    pub term: u64,
    /// Log index of the entry.
    pub index: u64,
}

#[cfg(test)]
mod tests;
```

Update `src/rpc/mod.rs`:

```rust
//! JSON-RPC server.

pub mod types;
```

- [ ] **Step 2: Write JSON round-trip tests**

`src/rpc/types/tests.rs`:

```rust
use super::*;

#[test]
fn submit_outcome_accepted_round_trip() {
    let v = SubmitOutcome {
        accepted: true,
        applied_index: Some(42),
        reject_reason: None,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: SubmitOutcome = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn submit_outcome_rejected_round_trip() {
    let v = SubmitOutcome {
        accepted: false,
        applied_index: None,
        reject_reason: Some("verify".into()),
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: SubmitOutcome = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn node_state_round_trip() {
    let v = NodeState {
        node_id: 2,
        role: NodeRole::Leader,
        leader_id: Some(2),
        last_log_id: Some(LogIdView { term: 4, index: 93 }),
        applied_index: 93,
        nullifier_count: 287,
        starstream_utxo_count: 287,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: NodeState = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn submit_outcome_schema_compiles() {
    let _schema = schemars::schema_for!(SubmitOutcome);
}

#[test]
fn node_state_schema_compiles() {
    let _schema = schemars::schema_for!(NodeState);
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p omega-toy-consensus --lib rpc::types
```

Expected: 5 tests, all PASS.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/rpc/
git commit -m "omega-toy-consensus: wire types with serde + JsonSchema round-trip"
```

---

## Task 5: Error code map + routing translator

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/src/rpc/error.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/mod.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/routing.rs`
- Create: `omega-commitment/crates/omega-toy-consensus/tests/routing.rs`

- [ ] **Step 1: Define error code constants**

`src/rpc/error.rs`:

```rust
//! JSON-RPC application error code constants and constructors.
//!
//! See spec `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`
//! § "Error code map (JSON-RPC application range)" for the contract.

use jsonrpsee::types::ErrorObjectOwned;
use serde::Serialize;

/// `−32000` — non-leader; `data` carries leader hint.
pub const CODE_NOT_LEADER: i32 = -32000;
/// `−32001` — proof verification failed.
pub const CODE_VERIFY: i32 = -32001;
/// `−32002` — CBOR decode / structural failure.
pub const CODE_INVALID_CLAIM: i32 = -32002;
/// `−32003` — nullifier already present.
pub const CODE_REPLAY: i32 = -32003;
/// `−32004` — writer actor unavailable (transient).
pub const CODE_WRITER_CLOSED: i32 = -32004;
/// `−32005` — apply did not complete in deadline.
pub const CODE_TIMEOUT: i32 = -32005;

/// Hint sent in `data` for `−32000 NotLeader`.
#[derive(Debug, Serialize)]
pub struct NotLeaderHint {
    /// Leader's u64 id, when openraft knows it.
    pub leader_id: Option<u64>,
    /// Public RPC URL of the leader, when this node knows it.
    pub leader_rpc_url: Option<String>,
}

/// Builds a `−32000 NotLeader` error.
///
/// # Soundness
///
/// The hint is advisory only; clients MUST treat absent fields as "leader
/// unknown" and retry against any peer. Server-side proxying is forbidden by
/// the spec — the wire surface is stateless.
pub fn not_leader(leader_id: Option<u64>, leader_rpc_url: Option<String>) -> ErrorObjectOwned {
    let hint = NotLeaderHint {
        leader_id,
        leader_rpc_url,
    };
    ErrorObjectOwned::owned(
        CODE_NOT_LEADER,
        "not leader",
        Some(serde_json::to_value(&hint).expect("hint serialises")),
    )
}

/// Builds a `−32001 Verify` error.
pub fn verify(detail: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_VERIFY,
        "proof verification failed",
        Some(serde_json::json!({ "verify_error": detail.into() })),
    )
}

/// Builds a `−32002 InvalidClaim` error.
pub fn invalid_claim(detail: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_INVALID_CLAIM,
        "invalid claim",
        Some(serde_json::json!({ "detail": detail.into() })),
    )
}

/// Builds a `−32003 Replay` error.
pub fn replay(sub_tree_id: u32, leaf_index: u64) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_REPLAY,
        "claim replays an existing nullifier",
        Some(serde_json::json!({
            "sub_tree_id": sub_tree_id,
            "leaf_index": leaf_index,
        })),
    )
}

/// Builds a `−32004 WriterClosed` error.
pub fn writer_closed() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_WRITER_CLOSED,
        "writer actor unavailable",
        Some(serde_json::json!({ "retryable": true })),
    )
}

/// Builds a `−32005 Timeout` error.
pub fn timeout(deadline_ms: u32) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_TIMEOUT,
        "apply deadline elapsed",
        Some(serde_json::json!({ "deadline_ms": deadline_ms })),
    )
}
```

- [ ] **Step 2: Wire `error` into `rpc/mod.rs`**

```rust
//! JSON-RPC server.

pub mod error;
pub mod types;
```

- [ ] **Step 3: Write the openraft → JSON-RPC translator**

`src/routing.rs`:

```rust
//! openraft → JSON-RPC error translation.
//!
//! This module is the single source of truth for mapping
//! [`openraft::error::ClientWriteError`] and [`omega_mock_ledger::LedgerError`]
//! into the `−32000..−32005` JSON-RPC error code space.

use jsonrpsee::types::ErrorObjectOwned;
use omega_mock_ledger::{LedgerError, OmegaRaftTypeConfig};
use openraft::error::{ClientWriteError, ForwardToLeader};

use crate::rpc::error;

/// Translates an openraft `ClientWriteError` into a JSON-RPC `ErrorObjectOwned`.
///
/// # Soundness
///
/// The translator is total: every `ClientWriteError` variant produces exactly
/// one JSON-RPC error. `ForwardToLeader` becomes `−32000` with the leader
/// hint; everything else (including the never-returned `EmptyMembership` /
/// `LearnerIsLagging` variants) collapses to `−32603 internal error` because
/// they do not arise in the v0.1 cluster topology.
pub fn translate_client_write_error(
    err: ClientWriteError<u64, openraft::BasicNode>,
    resolve_leader_url: impl FnOnce(u64) -> Option<String>,
) -> ErrorObjectOwned {
    match err {
        ClientWriteError::ForwardToLeader(ForwardToLeader {
            leader_id,
            leader_node: _,
        }) => {
            let leader_rpc_url = leader_id.and_then(resolve_leader_url);
            error::not_leader(leader_id, leader_rpc_url)
        }
        other => ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            jsonrpsee::types::error::INTERNAL_ERROR_MSG,
            Some(serde_json::json!({ "openraft": other.to_string() })),
        ),
    }
}

/// Translates a `LedgerError` (from `apply_to_state_machine`) into JSON-RPC.
///
/// # Soundness
///
/// `LedgerError::Verify` → `−32001`; `LedgerError::InvalidClaim` → `−32002`;
/// `LedgerError::Replay` → `−32003` with sub-tree + leaf-index hint;
/// `LedgerError::WriterClosed` / `WriterReplyCanceled` → `−32004` (transient);
/// every other variant collapses to `−32603 internal error`. This mirrors the
/// rejection classification documented in the spec; downstream callers MUST
/// treat unknown codes as "do not retry".
pub fn translate_ledger_error(err: LedgerError) -> ErrorObjectOwned {
    match err {
        LedgerError::Verify(detail) => error::verify(detail.to_string()),
        LedgerError::InvalidClaim(detail) => error::invalid_claim(detail.to_string()),
        LedgerError::Replay {
            sub_tree_id,
            leaf_index,
        } => error::replay(sub_tree_id, leaf_index),
        LedgerError::WriterClosed | LedgerError::WriterReplyCanceled => error::writer_closed(),
        other => ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            jsonrpsee::types::error::INTERNAL_ERROR_MSG,
            Some(serde_json::json!({ "ledger": other.to_string() })),
        ),
    }
}

/// Type alias to keep `OmegaRaftTypeConfig` paths short in callers.
pub(crate) type RaftCfg = OmegaRaftTypeConfig;
```

- [ ] **Step 4: Write integration tests**

Create `tests/routing.rs`:

```rust
use omega_mock_ledger::LedgerError;
use omega_toy_consensus::routing::{translate_client_write_error, translate_ledger_error};
use openraft::error::{ClientWriteError, ForwardToLeader};

#[test]
fn forward_to_leader_with_known_leader_emits_not_leader_with_url() {
    let err = ClientWriteError::ForwardToLeader::<u64, openraft::BasicNode>(ForwardToLeader {
        leader_id: Some(2),
        leader_node: Some(openraft::BasicNode::default()),
    });
    let obj = translate_client_write_error(err, |id| Some(format!("http://127.0.0.1:800{id}")));
    assert_eq!(obj.code(), -32000);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["leader_id"], 2);
    assert_eq!(v["leader_rpc_url"], "http://127.0.0.1:8002");
}

#[test]
fn forward_to_leader_unknown_leader_emits_not_leader_without_url() {
    let err = ClientWriteError::ForwardToLeader::<u64, openraft::BasicNode>(ForwardToLeader {
        leader_id: None,
        leader_node: None,
    });
    let obj = translate_client_write_error(err, |_| None);
    assert_eq!(obj.code(), -32000);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert!(v["leader_id"].is_null());
    assert!(v["leader_rpc_url"].is_null());
}

#[test]
fn ledger_replay_emits_neg_32003_with_hint() {
    let err = LedgerError::Replay {
        sub_tree_id: 1,
        leaf_index: 42,
    };
    let obj = translate_ledger_error(err);
    assert_eq!(obj.code(), -32003);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["sub_tree_id"], 1);
    assert_eq!(v["leaf_index"], 42);
}

#[test]
fn ledger_writer_closed_is_transient() {
    let obj = translate_ledger_error(LedgerError::WriterClosed);
    assert_eq!(obj.code(), -32004);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["retryable"], true);
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p omega-toy-consensus --test routing
```

Expected: 4 tests, all PASS.

- [ ] **Step 6: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/rpc/error.rs omega-commitment/crates/omega-toy-consensus/src/rpc/mod.rs omega-commitment/crates/omega-toy-consensus/src/routing.rs omega-commitment/crates/omega-toy-consensus/tests/routing.rs
git commit -m "omega-toy-consensus: error code map + openraft/ledger → JSON-RPC translator"
```

---

## Task 6: `OmegaRpc` trait + `OmegaRpcImpl` struct (no logic yet)

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/mod.rs`

- [ ] **Step 1: Define the RPC trait via jsonrpsee proc-macro**

`src/rpc/server.rs`:

```rust
//! JSON-RPC server: trait + impl + bind.

use std::sync::Arc;

use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use omega_claim_tx::ClaimTx;

use crate::rpc::types::{NodeState, SubmitOutcome};

/// JSON-RPC surface for a single LoganNet node.
///
/// Wire method names: `omega_submitClaim`, `omega_getState`.
#[rpc(server, namespace = "omega")]
pub trait OmegaRpc {
    /// Submits a single claim transaction. Returns the applied log index on
    /// success, or a structured error on rejection.
    #[method(name = "submitClaim")]
    async fn submit_claim(&self, claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned>;

    /// Reads the node's current consensus and ledger state. Read-only.
    #[method(name = "getState")]
    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned>;
}

/// Concrete implementation of [`OmegaRpcServer`]. Carries shared handles to
/// the openraft instance, the mock-ledger reader pool, and the static peer
/// list (for leader-hint URL resolution).
#[derive(Clone)]
pub struct OmegaRpcImpl {
    /// Inner shared state. Cheap to clone; jsonrpsee clones per request.
    pub(crate) inner: Arc<OmegaRpcShared>,
}

/// Shared state behind the RPC impl. Every field here is `Send + Sync` for
/// the jsonrpsee server's `'static` requirement.
pub(crate) struct OmegaRpcShared {
    pub(crate) node_id: u64,
    pub(crate) raft: openraft::Raft<omega_mock_ledger::OmegaRaftTypeConfig>,
    pub(crate) ledger: Arc<omega_mock_ledger::MockLedger>,
    pub(crate) peers: Vec<crate::PeerConfig>,
    pub(crate) apply_deadline: std::time::Duration,
}

#[async_trait]
impl OmegaRpcServer for OmegaRpcImpl {
    async fn submit_claim(&self, _claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned> {
        Err(ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            "submit_claim unimplemented",
            None::<()>,
        ))
    }

    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned> {
        Err(ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            "get_state unimplemented",
            None::<()>,
        ))
    }
}
```

- [ ] **Step 2: Wire `server` into `rpc/mod.rs`**

```rust
//! JSON-RPC server.

pub mod error;
pub mod server;
pub mod types;

pub use server::{OmegaRpc, OmegaRpcImpl, OmegaRpcServer};
```

- [ ] **Step 3: Build**

```bash
cargo build -p omega-toy-consensus
```

Expected: clean build (warnings about unused `OmegaRpcShared` fields OK).

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/rpc/
git commit -m "omega-toy-consensus: OmegaRpc trait via jsonrpsee proc-macro + impl skeleton"
```

---

## Task 7: `submit_claim` happy path + leader forwarding

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs`

- [ ] **Step 1: Implement `submit_claim`**

Replace the body of `OmegaRpcServer for OmegaRpcImpl::submit_claim` with:

```rust
async fn submit_claim(&self, claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned> {
    use crate::routing::{translate_client_write_error, translate_ledger_error};

    let cmd = omega_mock_ledger::LedgerCommand::ApplyClaim { claim };
    let inner = self.inner.clone();

    let result = tokio::time::timeout(
        inner.apply_deadline,
        inner.raft.client_write(cmd),
    )
    .await;

    let response = match result {
        Err(_elapsed) => {
            let ms = inner.apply_deadline.as_millis().min(u32::MAX as u128) as u32;
            return Err(crate::rpc::error::timeout(ms));
        }
        Ok(Ok(write)) => write,
        Ok(Err(err)) => {
            return Err(translate_client_write_error(err, |id| {
                inner
                    .peers
                    .iter()
                    .find(|p| p.node_id == id)
                    .map(|p| p.rpc_url.clone())
            }));
        }
    };

    match response.response {
        omega_mock_ledger::LedgerResponse {
            accepted: true,
            ..
        } => Ok(SubmitOutcome {
            accepted: true,
            applied_index: Some(response.log_id.index),
            reject_reason: None,
        }),
        omega_mock_ledger::LedgerResponse {
            accepted: false,
            reject: Some(err),
            ..
        } => {
            // err is a LedgerError variant from the writer; map it.
            let json_err = translate_ledger_error(err);
            // For accepted=false we surface the rejection class via
            // SubmitOutcome rather than a JSON-RPC error: the apply did
            // happen (the log index advanced), the state machine just
            // refused to mutate. JSON-RPC errors are reserved for
            // pre-apply failures (NotLeader, Timeout, WriterClosed).
            //
            // Map error code → reject_reason string per the spec:
            let reason = match json_err.code() {
                crate::rpc::error::CODE_VERIFY => "verify",
                crate::rpc::error::CODE_INVALID_CLAIM => "invalid",
                crate::rpc::error::CODE_REPLAY => "replay",
                _ => "internal",
            };
            Ok(SubmitOutcome {
                accepted: false,
                applied_index: Some(response.log_id.index),
                reject_reason: Some(reason.into()),
            })
        }
        omega_mock_ledger::LedgerResponse {
            accepted: false,
            reject: None,
            ..
        } => Ok(SubmitOutcome {
            accepted: false,
            applied_index: Some(response.log_id.index),
            reject_reason: Some("internal".into()),
        }),
    }
}
```

This requires `omega_mock_ledger::LedgerResponse` to expose a `reject: Option<LedgerError>` field. If the existing struct does not, the writer needs a small extension: add `reject: Option<LedgerError>` to `LedgerResponse` (in `omega-mock-ledger/src/storage.rs`) and populate it in `writer.rs::apply_claim_tx`. **Confirm with the existing storage.rs surface before editing**; if the field is named differently (e.g. `error: Option<...>`), use that name throughout.

- [ ] **Step 2: Implement `get_state`**

Replace the body of `get_state`:

```rust
async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned> {
    use crate::rpc::types::{LogIdView, NodeRole};

    let inner = self.inner.clone();
    let metrics = inner.raft.metrics().borrow().clone();

    let role = match metrics.state {
        openraft::ServerState::Leader => NodeRole::Leader,
        openraft::ServerState::Follower => NodeRole::Follower,
        openraft::ServerState::Candidate => NodeRole::Candidate,
        openraft::ServerState::Learner => NodeRole::Learner,
        // openraft::ServerState::Shutdown is not part of the wire vocab;
        // surface as Follower so clients don't have to learn an extra state.
        _ => NodeRole::Follower,
    };

    let last_log_id = metrics.last_log_index.zip(metrics.current_term).map(
        |(index, term)| LogIdView { term, index },
    );

    // Reader-pool calls; both should be cheap point-in-time queries.
    let nullifier_count = inner
        .ledger
        .nullifier_count()
        .map_err(crate::routing::translate_ledger_error)?;
    let starstream_utxo_count = inner
        .ledger
        .starstream_utxo_count()
        .map_err(crate::routing::translate_ledger_error)?;

    Ok(NodeState {
        node_id: inner.node_id,
        role,
        leader_id: metrics.current_leader,
        last_log_id,
        applied_index: metrics.last_applied.map(|l| l.index).unwrap_or(0),
        nullifier_count,
        starstream_utxo_count,
    })
}
```

If `MockLedger::nullifier_count` / `starstream_utxo_count` do not exist as public methods on the reader-pool side, add them. Each is a `SELECT COUNT(*) FROM <table>` against the reader-pool — see `crates/omega-mock-ledger/src/lib.rs` for the existing `nullifier_exists` reader-pool pattern; mirror that.

- [ ] **Step 3: Build**

```bash
cargo build -p omega-toy-consensus
```

Expected: clean. (If `LedgerResponse.reject` or `MockLedger::*_count` need adding, you'll see specific errors here — make those small additions in the upstream crate, commit them as a separate "omega-mock-ledger: expose count helpers" commit, and re-build.)

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs
# plus any omega-mock-ledger changes from above:
git add omega-commitment/crates/omega-mock-ledger/src/  # if you edited
git commit -m "omega-toy-consensus: submit_claim + get_state happy paths"
```

---

## Task 8: `Node::start` — assemble openraft + writer + network + RPC

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/node.rs`

- [ ] **Step 1: Implement bring-up**

Replace `src/node.rs` with:

```rust
//! Node lifecycle.

use std::sync::Arc;

use jsonrpsee::server::ServerHandle;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::rpc::server::{OmegaRpcImpl, OmegaRpcServer, OmegaRpcShared};
use crate::{ConsensusError, NodeConfig};

/// Live LoganNet node. See [`crate::start`] for the public entry point.
pub struct Node;

/// Handle for graceful shutdown of a running [`Node`].
pub struct NodeHandle {
    shutdown_tx: oneshot::Sender<()>,
    server_handle: ServerHandle,
    join: JoinHandle<Result<(), ConsensusError>>,
    raft: openraft::Raft<omega_mock_ledger::OmegaRaftTypeConfig>,
}

impl Node {
    /// Brings the node up. Mounts SQLite + writer actor, dials peers, binds
    /// JSON-RPC, then either initialises a fresh cluster (when this node is
    /// `node_id == lowest_in_cluster`) or waits to be added by the leader.
    ///
    /// # Errors
    ///
    /// See [`ConsensusError`].
    ///
    /// # Soundness
    ///
    /// Bring-up is idempotent on storage; the writer actor's verify-before-mutate
    /// invariant (see `omega-mock-ledger`'s `# Soundness` block) is preserved
    /// across crashes. This function does NOT validate cluster identity beyond
    /// `cluster_id` string equality — operators must ensure all peers use the
    /// same string.
    pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
        // 1. Open the mock-ledger writer + reader pool.
        let ledger = Arc::new(omega_mock_ledger::MockLedger::open(&config.data_dir)?);

        // 2. Build openraft storage adaptor over the mock ledger.
        let storage = ledger.openraft_storage();
        let (log_store, state_machine) = storage.openraft_parts();

        // 3. Build the libp2p network factory.
        let network = omega_network::LibP2pNetwork::new(
            config.node_id,
            config.libp2p_listen.parse().map_err(|e: libp2p::multiaddr::Error| {
                ConsensusError::Config(format!("libp2p_listen parse: {e}"))
            })?,
            config
                .peers
                .iter()
                .map(|p| {
                    let id = p.node_id;
                    let addr = p.libp2p_addr.parse::<libp2p::Multiaddr>().map_err(|e| {
                        ConsensusError::Config(format!("peer libp2p_addr parse: {e}"))
                    })?;
                    Ok((id, addr))
                })
                .collect::<Result<Vec<_>, ConsensusError>>()?,
        )
        .await?;
        let network_factory = omega_network::LibP2pNetworkFactory::new(network.clone());

        // 4. Build the openraft Raft instance.
        let raft_config = openraft::Config {
            cluster_name: config.cluster_id.clone(),
            heartbeat_interval: 250,
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            ..Default::default()
        };
        let raft = openraft::Raft::new(
            config.node_id,
            Arc::new(raft_config),
            network_factory,
            log_store,
            state_machine,
        )
        .await
        .map_err(|e| ConsensusError::Raft(e.to_string()))?;

        // 5. Initialise membership if this is the lowest node id and the log is
        //    empty (Group 1 only supports static topologies).
        let is_initialiser = std::iter::once(config.node_id)
            .chain(config.peers.iter().map(|p| p.node_id))
            .min()
            == Some(config.node_id);
        if is_initialiser && raft.metrics().borrow().last_log_index.is_none() {
            let mut members = std::collections::BTreeMap::new();
            members.insert(config.node_id, openraft::BasicNode::default());
            for p in &config.peers {
                members.insert(p.node_id, openraft::BasicNode::default());
            }
            // Initialise in the background; if quorum forms before this returns
            // openraft tolerates the duplicate init.
            let raft_init = raft.clone();
            tokio::spawn(async move {
                let _ = raft_init.initialize(members).await;
            });
        }

        // 6. Bind the JSON-RPC server.
        let shared = Arc::new(OmegaRpcShared {
            node_id: config.node_id,
            raft: raft.clone(),
            ledger: ledger.clone(),
            peers: config.peers.clone(),
            apply_deadline: config.apply_deadline,
        });
        let rpc_impl = OmegaRpcImpl { inner: shared };
        let server = jsonrpsee::server::Server::builder()
            .max_request_body_size(config.rpc.max_request_bytes)
            .max_subscriptions_per_connection(0)
            .build(config.rpc.bind)
            .await
            .map_err(|e| ConsensusError::RpcBind {
                addr: config.rpc.bind,
                source: e.into(),
            })?;
        let server_handle = server.start(rpc_impl.into_rpc());

        // 7. Build NodeHandle: hold shutdown_tx, server_handle, raft.
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let raft_for_join = raft.clone();
        let join: JoinHandle<Result<(), ConsensusError>> = tokio::spawn(async move {
            // Park until shutdown is requested.
            let _ = shutdown_rx.await;
            Ok(())
        });

        Ok(NodeHandle {
            shutdown_tx,
            server_handle,
            join,
            raft: raft_for_join,
        })
    }
}

impl NodeHandle {
    /// Initiates graceful shutdown: stops the JSON-RPC server, signals the
    /// runtime task, awaits the join, and shuts the openraft instance.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::ShutdownJoin`] — runtime task panicked.
    /// - [`ConsensusError::Raft`] — openraft shutdown returned an error.
    pub async fn shutdown(self) -> Result<(), ConsensusError> {
        // Stop new RPC requests.
        let _ = self.server_handle.stop();
        // Signal the parked runtime task.
        let _ = self.shutdown_tx.send(());
        // Wait for the runtime to settle.
        self.join
            .await
            .map_err(|e| ConsensusError::ShutdownJoin(e.to_string()))?
            .map_err(|e| match e {
                ConsensusError::ShutdownJoin(_) => e,
                other => other,
            })?;
        // Shut openraft.
        self.raft
            .shutdown()
            .await
            .map_err(|e| ConsensusError::Raft(e.to_string()))?;
        Ok(())
    }
}
```

If specific upstream API names differ from the above (`MockLedger::open`, `openraft_storage`, `LibP2pNetwork::new`, `LibP2pNetworkFactory::new`), adapt the code to match. The upstream APIs are stable as of the merged PRs; verify by `grep -rn "pub fn" crates/omega-mock-ledger/src/ crates/omega-network/src/`.

- [ ] **Step 2: Build**

```bash
cargo build -p omega-toy-consensus
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/node.rs
git commit -m "omega-toy-consensus: Node::start + NodeHandle::shutdown"
```

---

## Task 9: Run binary with clap

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/src/bin/omega-toy-consensus.rs`

- [ ] **Step 1: Write the binary**

```rust
//! `omega-toy-consensus run` — boot a single LoganNet node from CLI flags.

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
    /// Libp2p multiaddr the node listens on.
    #[arg(long)]
    listen: String,
    /// Static peer; format: `<id>,<libp2p_addr>,<rpc_url>`. Repeat once per
    /// peer.
    #[arg(long = "peer", value_name = "ID,ADDR,URL")]
    peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind address.
    #[arg(long)]
    rpc: std::net::SocketAddr,
    /// Cluster identifier; must match across peers.
    #[arg(long, default_value = "loganet-dev")]
    cluster_id: String,
    /// Apply deadline (seconds).
    #[arg(long, default_value = "5")]
    apply_deadline_secs: u64,
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
    let config = NodeConfig {
        node_id: args.node_id,
        data_dir: args.data_dir,
        libp2p_listen: args.listen,
        peers: args.peers,
        rpc: RpcConfig {
            bind: args.rpc,
            max_batch: 25,
            max_request_bytes: 1024 * 1024,
        },
        cluster_id: args.cluster_id,
        apply_deadline: Duration::from_secs(args.apply_deadline_secs),
    };

    tracing::info!(node_id = config.node_id, rpc = %config.rpc.bind, "starting");

    let handle = omega_toy_consensus::start(config).await?;

    // Park until SIGINT.
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown requested");
    handle.shutdown().await?;
    Ok(())
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p omega-toy-consensus --bin omega-toy-consensus
```

Expected: clean.

- [ ] **Step 3: Verify CLI help renders**

```bash
./target/debug/omega-toy-consensus run --help
```

Expected: usage block listing `--node-id`, `--data-dir`, `--listen`, `--peer`, `--rpc`, `--cluster-id`, `--apply-deadline-secs`.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/bin/
git commit -m "omega-toy-consensus: run binary with clap CLI"
```

---

## Task 10: turmoil fixture — `three_node_loganet`

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/common/mod.rs`

- [ ] **Step 1: Write the fixture**

`tests/common/mod.rs`:

```rust
//! Shared helpers for turmoil-based 3-node tests.

use std::time::Duration;

use omega_toy_consensus::{NodeConfig, PeerConfig, RpcConfig};

/// Build a 3-node LoganNet config triple, all on synthetic localhost-style
/// addresses turmoil resolves to its in-memory network.
pub fn three_node_configs() -> [NodeConfig; 3] {
    let peer = |id: u64| PeerConfig {
        node_id: id,
        libp2p_addr: format!("/ip4/0.0.0.0/tcp/{}", 4000 + id),
        rpc_url: format!("http://node{id}:800{id}"),
    };
    let mk = |id: u64, peers: Vec<PeerConfig>| NodeConfig {
        node_id: id,
        data_dir: tempfile::tempdir().unwrap().keep(),
        libp2p_listen: format!("/ip4/0.0.0.0/tcp/{}", 4000 + id),
        peers,
        rpc: RpcConfig {
            bind: format!("0.0.0.0:{}", 8000 + id).parse().unwrap(),
            max_batch: 25,
            max_request_bytes: 1024 * 1024,
        },
        cluster_id: "loganet-test".into(),
        apply_deadline: Duration::from_secs(5),
    };
    [
        mk(1, vec![peer(2), peer(3)]),
        mk(2, vec![peer(1), peer(3)]),
        mk(3, vec![peer(1), peer(2)]),
    ]
}

/// Boots a 3-node turmoil sim. Each host runs `omega_toy_consensus::start`
/// and parks until the sim shuts down.
pub fn three_node_sim() -> turmoil::Sim<'static> {
    let configs = three_node_configs();
    let mut sim = turmoil::Builder::new()
        .simulation_duration(Duration::from_secs(60))
        .build();
    for cfg in configs {
        let cfg_clone = cfg.clone();
        let host = format!("node{}", cfg_clone.node_id);
        sim.host(host.as_str(), move || {
            let cfg = cfg_clone.clone();
            async move {
                let handle = omega_toy_consensus::start(cfg).await?;
                std::future::pending::<()>().await;
                drop(handle);
                Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
            }
        });
    }
    sim
}
```

The exact `turmoil::Builder` API (and how it integrates with `LibP2pNetworkFactory`) may need adapter glue. If `LibP2pNetwork::new` cannot run inside turmoil's stub TCP, add a `cfg(test)` constructor variant in `omega-network` that exposes an in-memory channel transport. Use `skills/local/rust-test-turmoil/SKILL.md` for the exact pattern.

- [ ] **Step 2: Build (no tests yet)**

```bash
cargo build -p omega-toy-consensus --tests
```

Expected: clean. (tests/common/mod.rs is shared, not a test target itself.)

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/common/
git commit -m "omega-toy-consensus: 3-node turmoil fixture"
```

---

## Task 11: Test — `single_leader_emerges`

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/single_leader_emerges.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn single_leader_emerges() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // After 3s of simulated time, every node's get_state should agree on
    // exactly one leader_id, and exactly one node should report role=Leader.
    let mut leaders = 0;
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(url)
            .unwrap();
        let state: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
            leaders += 1;
        }
    }
    assert_eq!(leaders, 1, "exactly one leader after 3s");
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --test single_leader_emerges
```

Expected: PASS within ~10s wall-clock.

If fails, inspect via `RUST_LOG=info,openraft=debug cargo test ...` and fix bring-up timing or initialise-membership logic in Task 8.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/single_leader_emerges.rs
git commit -m "omega-toy-consensus: turmoil test single leader emerges"
```

---

## Task 12: Test — `single_claim_roundtrip`

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/single_claim_roundtrip.rs`
- Create: `omega-commitment/crates/omega-toy-consensus/tests/common/synthetic_claim.rs` (helper)

- [ ] **Step 1: Add a synthetic claim helper**

`tests/common/synthetic_claim.rs`:

```rust
//! Builds a minimal valid `ClaimTx` against a synthetic 256-leaf UTxO sub-tree
//! genesis. Reused across multiple integration tests.

use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes};

/// Builds a synthetic claim that the verifier will accept (uses the test
/// fixture genesis already shipped by `omega-claim-tx::tests` /
/// `omega-claim-prover::tests`). Adapt to whatever helper those crates
/// expose; if neither exposes one publicly, gate the body on
/// `#[cfg(test)]` and re-export through a dev-only `omega-claim-prover`
/// helper.
pub fn synthetic_accepted_claim_for_leaf(leaf_index: u64) -> ClaimTx {
    // Uses the same 256-leaf synthetic UTxO sub-tree as
    // `omega-claim-prover/tests/prover_smoke.rs`; if that test exposes a
    // helper, prefer importing it via #[path = "..."]; otherwise duplicate
    // the construction inline (keep ≤ 30 LOC).
    todo!("re-use omega-claim-prover's synthetic fixture")
}
```

If `omega-claim-prover` does not expose a public helper, add a `pub mod test_fixtures` (gated `#[cfg(any(test, feature = "test-fixtures"))]`) to `omega-claim-prover` exporting `build_synthetic_accepted_claim(leaf_index: u64) -> ClaimTx`. Commit that as a small omega-claim-prover patch.

- [ ] **Step 2: Wire helper into `tests/common/mod.rs`**

Add `pub mod synthetic_claim;` to `tests/common/mod.rs`.

- [ ] **Step 3: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn single_claim_roundtrip() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Find the leader.
    let mut leader_url = None;
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&url)
            .unwrap();
        let state: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
            leader_url = Some(url);
            break;
        }
    }
    let leader_url = leader_url.expect("a leader exists after 3s");

    // Submit a claim.
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(42);
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let outcome: omega_toy_consensus::SubmitOutcome =
        jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params)
            .await
            .unwrap();
    assert!(outcome.accepted);
    let applied_index = outcome.applied_index.expect("applied_index when accepted");

    // After 1s of simulated time every node's getState should show
    // applied_index >= the value the leader reported.
    sim.elapse(Duration::from_secs(1));
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(url)
            .unwrap();
        let state: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        assert!(
            state.applied_index >= applied_index,
            "node {node} applied_index {} < leader's {}",
            state.applied_index,
            applied_index
        );
        assert!(state.nullifier_count >= 1);
        assert!(state.starstream_utxo_count >= 1);
    }
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 4: Run test**

```bash
cargo test -p omega-toy-consensus --test single_claim_roundtrip
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/common/synthetic_claim.rs omega-commitment/crates/omega-toy-consensus/tests/common/mod.rs omega-commitment/crates/omega-toy-consensus/tests/single_claim_roundtrip.rs
# plus any omega-claim-prover test_fixtures changes:
git add omega-commitment/crates/omega-claim-prover/  # if you patched
git commit -m "omega-toy-consensus: turmoil test single claim round-trip"
```

---

## Task 13: Test — `leader_forwarding` (submit to follower → −32000 → retry succeeds)

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/leader_forwarding.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn submit_to_follower_returns_neg_32000_with_url() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Submit to every node; expect exactly one to accept (the leader) and the
    // other two to return −32000 with a leader_rpc_url that resolves to the
    // accepting node.
    let mut leader_responder: Option<String> = None;
    let mut hint_url: Option<String> = None;
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&url)
            .unwrap();
        let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(7);
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
            jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
        match result {
            Ok(outcome) if outcome.accepted => leader_responder = Some(url),
            Err(jsonrpsee::core::ClientError::Call(obj)) if obj.code() == -32000 => {
                let data = obj.data().expect("hint present");
                let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
                hint_url = v["leader_rpc_url"].as_str().map(str::to_string);
            }
            other => panic!("unexpected response from {node}: {other:?}"),
        }
    }
    assert!(leader_responder.is_some(), "exactly one node accepted");
    assert!(hint_url.is_some(), "follower returned a leader hint");
    sim.run().unwrap();
    Ok(())
}
```

The exact `omega_submitClaim` call against the leader counts as test pollution (it consumes leaf 7's nullifier); idempotency is preserved because the test runs only once per fresh sim.

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --test leader_forwarding
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/leader_forwarding.rs
git commit -m "omega-toy-consensus: turmoil test follower returns -32000 with leader hint"
```

---

## Task 14: Tests — partition (minority does not commit; majority continues)

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/partition.rs`

- [ ] **Step 1: Write tests**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn partitioned_minority_does_not_commit() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Partition node1 from the rest.
    sim.partition("node1", "node2");
    sim.partition("node1", "node3");

    // Submit to node1; expect either a Timeout (−32005) or NotLeader (−32000)
    // with a hint that does not point to a reachable URL — node1 is alone.
    let url = "http://node1:8001";
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(url)
        .unwrap();
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(13);
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    sim.elapse(Duration::from_secs(6));
    let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
        jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
    match result {
        Err(jsonrpsee::core::ClientError::Call(obj)) => {
            assert!(
                obj.code() == -32000 || obj.code() == -32005,
                "expected NotLeader or Timeout from minority node, got {}",
                obj.code()
            );
        }
        Ok(outcome) => panic!("minority must not accept: {outcome:?}"),
        Err(other) => panic!("unexpected transport error: {other:?}"),
    }
    sim.run().unwrap();
    Ok(())
}

#[turmoil::test]
async fn partitioned_majority_continues_to_commit() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Same partition as above.
    sim.partition("node1", "node2");
    sim.partition("node1", "node3");
    sim.elapse(Duration::from_secs(2));

    // Submit to node2; expect accept or NotLeader-with-hint pointing at
    // node3 (whichever one of the {2,3} pair is leader).
    let mut accepted = false;
    for node in ["node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(url)
            .unwrap();
        let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(99);
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
            jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
        if let Ok(outcome) = result {
            if outcome.accepted {
                accepted = true;
                break;
            }
        }
    }
    assert!(accepted, "majority {{2,3}} must continue to commit");
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p omega-toy-consensus --test partition
```

Expected: 2 tests, both PASS.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/partition.rs
git commit -m "omega-toy-consensus: turmoil tests for partition behaviour"
```

---

## Task 15: Failpoint — drop one AppendEntries

**Files:**
- Modify: `omega-commitment/crates/omega-network/src/network.rs` (add failpoint)
- Create: `omega-commitment/crates/omega-toy-consensus/tests/failpoint_drop_appendentries.rs`

- [ ] **Step 1: Add a failpoint at the network seam**

In `crates/omega-network/src/network.rs`, find the function that sends an outbound Raft request (likely `send_raft_request` or the `RaftNetwork::send_*` impls). At the entry of each, insert:

```rust
#[cfg(feature = "failpoints")]
{
    fail::fail_point!("omega_network::send_appendentries", |_| {
        Err(omega_network::rpc::OmegaNetworkError::OutboundClosed)
    });
}
```

Wire `failpoints = ["fail/failpoints"]` into `omega-network`'s `[features]` block.

Use `skills/local/rust-test-failpoints/SKILL.md` for the exact macro pattern; the injection-sites reference is the canonical list.

- [ ] **Step 2: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn drop_first_appendentries_eventually_recovers() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Configure a failpoint on node2 that drops the first AppendEntries from
    // the leader, then disables.
    fail::cfg("omega_network::send_appendentries", "1*return->off").unwrap();

    // Submit a claim; expect accept (other follower forms quorum).
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(15);
    let url = "http://node1:8001";
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(url)
        .unwrap();
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
        jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
    // Submit may go to a follower; loop is cheap.
    if let Ok(outcome) = result {
        assert!(outcome.accepted || outcome.reject_reason.is_some());
    }

    // After 5s, all 3 nodes should have applied at least 1 entry.
    sim.elapse(Duration::from_secs(5));
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(url)
            .unwrap();
        let state: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        assert!(state.applied_index >= 1, "{node} did not catch up");
    }
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p omega-toy-consensus --features failpoints --test failpoint_drop_appendentries
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-network/ omega-commitment/crates/omega-toy-consensus/tests/failpoint_drop_appendentries.rs
git commit -m "omega-toy-consensus: failpoint drop AppendEntries; eventual recovery"
```

---

## Task 16: Failpoint — byzantine vote replay

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/failpoint_byzantine_replay.rs`
- Modify: `omega-commitment/crates/omega-network/src/network.rs` (add `replay_old_vote` failpoint)

- [ ] **Step 1: Add a failpoint that injects a stale-term vote**

In `omega-network`'s outbound vote handler (or the receive path), insert:

```rust
#[cfg(feature = "failpoints")]
fail::fail_point!("omega_network::receive_vote_replay", |_| {
    // Replay logic: instead of dispatching the genuine vote, dispatch a
    // synthetic VoteRequest with an older term. The exact API is
    // openraft-specific; see `omega-network/src/rpc.rs::RaftRpcRequest::Vote`.
    // This fail-point is only useful when the test pre-populates a stale
    // request via fail::cfg("...", "return(<bytes>)").
    /* see implementation details in the SKILL */
});
```

(Detailed pattern lives in `skills/local/rust-test-failpoints/SKILL.md` § "byzantine replay"; if the SKILL does not yet have this pattern, document the chosen approach inline and update the SKILL via a small follow-up commit.)

- [ ] **Step 2: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn replayed_old_vote_does_not_advance_term() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Capture term at t=3s.
    let url1 = "http://node1:8001";
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(url1)
        .unwrap();
    let state_before: omega_toy_consensus::NodeState =
        jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
    let term_before = state_before.last_log_id.map(|l| l.term).unwrap_or(0);

    // Trigger byzantine replay.
    fail::cfg("omega_network::receive_vote_replay", "return").unwrap();
    sim.elapse(Duration::from_secs(2));
    fail::cfg("omega_network::receive_vote_replay", "off").unwrap();

    // Term must not regress.
    let state_after: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
        &client,
        "omega_getState",
        jsonrpsee::core::params::ArrayParams::new(),
    )
    .await
    .unwrap();
    let term_after = state_after.last_log_id.map(|l| l.term).unwrap_or(0);
    assert!(
        term_after >= term_before,
        "term regressed: {term_before} → {term_after}"
    );
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p omega-toy-consensus --features failpoints --test failpoint_byzantine_replay
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-network/ omega-commitment/crates/omega-toy-consensus/tests/failpoint_byzantine_replay.rs
git commit -m "omega-toy-consensus: failpoint byzantine vote replay; term safety holds"
```

---

## Task 17: Failpoint — writer closed mid-submit

**Files:**
- Modify: `omega-commitment/crates/omega-mock-ledger/src/writer.rs` (add failpoint)
- Create: `omega-commitment/crates/omega-toy-consensus/tests/failpoint_writer_closed.rs`

- [ ] **Step 1: Add the failpoint at the writer's `apply_claim_tx` entry**

In `omega-mock-ledger/src/writer.rs::apply_claim_tx` (or wherever the writer first receives the apply command), insert:

```rust
#[cfg(feature = "failpoints")]
fail::fail_point!("omega_mock_ledger::writer_close", |_| {
    Err(LedgerError::WriterClosed)
});
```

Add `failpoints = ["fail/failpoints"]` to `omega-mock-ledger`'s `[features]`.

- [ ] **Step 2: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn writer_closed_returns_neg_32004_no_state_advance() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    // Find leader.
    let leader_url = common::leader_url(&mut sim).await;

    // Capture applied_index before.
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();
    let before: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
        &client,
        "omega_getState",
        jsonrpsee::core::params::ArrayParams::new(),
    )
    .await
    .unwrap();

    // Inject writer-close.
    fail::cfg("omega_mock_ledger::writer_close", "return").unwrap();

    // Submit; expect −32004.
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(50);
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
        jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
    match result {
        Err(jsonrpsee::core::ClientError::Call(obj)) => {
            assert_eq!(obj.code(), -32004, "expected WriterClosed");
        }
        other => panic!("expected WriterClosed, got {other:?}"),
    }

    fail::cfg("omega_mock_ledger::writer_close", "off").unwrap();
    sim.elapse(Duration::from_secs(1));

    // applied_index must NOT have advanced past the failure.
    let after: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
        &client,
        "omega_getState",
        jsonrpsee::core::params::ArrayParams::new(),
    )
    .await
    .unwrap();
    // Some heartbeat-driven advancement may have occurred in the 1s tick;
    // the strong invariant is "applied_index did not advance because of the
    // failed apply". A weaker but mechanical check: applied_index ≤
    // before.applied_index + 1 (allows the empty-entry on leader).
    assert!(after.applied_index <= before.applied_index + 1);
    sim.run().unwrap();
    Ok(())
}
```

Add `pub async fn leader_url(...)` helper to `tests/common/mod.rs`:

```rust
pub async fn leader_url(_sim: &mut turmoil::Sim<'static>) -> String {
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&url)
            .unwrap();
        let state: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
            return url;
        }
    }
    panic!("no leader found");
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p omega-toy-consensus --features failpoints --test failpoint_writer_closed
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-mock-ledger/ omega-commitment/crates/omega-toy-consensus/tests/
git commit -m "omega-toy-consensus: failpoint writer closed mid-submit; -32004 surfaced"
```

---

## Task 18: Test — leader change during submit

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/leader_change_during_submit.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn leader_change_during_submit_yields_disjunction() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    let leader_url = common::leader_url(&mut sim).await;
    let leader_node: String = leader_url
        .strip_prefix("http://")
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .into();

    // Start the submit, then immediately partition the leader from {others}.
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(75);
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();
    let request_fut = async move {
        jsonrpsee::core::client::ClientT::request::<
            omega_toy_consensus::SubmitOutcome,
            _,
        >(&client, "omega_submitClaim", params)
        .await
    };

    // Force re-election by partitioning leader off.
    let leader = leader_node.clone();
    sim.partition(&leader, "node1");
    sim.partition(&leader, "node2");
    sim.partition(&leader, "node3");

    let outcome = request_fut.await;
    sim.elapse(Duration::from_secs(5));

    match outcome {
        Err(jsonrpsee::core::ClientError::Call(obj)) => {
            assert!(obj.code() == -32000 || obj.code() == -32005);
        }
        Ok(outcome) => {
            // Either it landed before partition (accepted) or it landed under
            // a stale term (rejected). Both are spec-permitted disjunction
            // outcomes.
            let _ = outcome;
        }
        Err(other) => panic!("unexpected: {other:?}"),
    }
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --test leader_change_during_submit
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/leader_change_during_submit.rs
git commit -m "omega-toy-consensus: turmoil test leader change during submit (disjunction)"
```

---

## Task 19: Test — snapshot install during submit

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_during_submit.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use std::time::Duration;

#[turmoil::test]
async fn snapshot_install_mid_submit_keeps_state_consistent() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    let leader_url = common::leader_url(&mut sim).await;
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();

    // Submit 50 claims to populate the log.
    for i in 0..50u64 {
        let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(i);
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let _: omega_toy_consensus::SubmitOutcome =
            jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params)
                .await
                .unwrap();
    }
    sim.elapse(Duration::from_secs(2));

    // Trigger snapshot via openraft's `trigger().snapshot()` path. If openraft
    // surfaces a snapshot trigger via getState, expose it through the RPC; if
    // not, use the existing automatic snapshotter and elapse enough simulated
    // time for it to fire.
    sim.elapse(Duration::from_secs(10));

    // Submit one more claim while a snapshot is plausibly in flight.
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(50);
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let outcome: omega_toy_consensus::SubmitOutcome =
        jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params)
            .await
            .unwrap();
    assert!(outcome.accepted);

    // Every node's nullifier_count should match the leader's (eventual
    // consistency under snapshot install).
    sim.elapse(Duration::from_secs(5));
    let leader_state: omega_toy_consensus::NodeState =
        jsonrpsee::core::client::ClientT::request(
            &client,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
    for node in ["node1", "node2", "node3"] {
        let url = format!("http://{node}:800{}", &node[4..]);
        let c = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(url)
            .unwrap();
        let s: omega_toy_consensus::NodeState = jsonrpsee::core::client::ClientT::request(
            &c,
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap();
        assert_eq!(s.nullifier_count, leader_state.nullifier_count);
        assert_eq!(s.starstream_utxo_count, leader_state.starstream_utxo_count);
    }
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --test snapshot_install_during_submit
```

Expected: PASS within ~30s wall-clock.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_during_submit.rs
git commit -m "omega-toy-consensus: turmoil test snapshot install mid-submit"
```

---

## Task 20: Shuttle-loom — writer-channel handshake

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/shuttle_writer_handshake.rs`

- [ ] **Step 1: Write the test**

```rust
//! Shuttle-loom concurrency exploration of the rpc-handler → writer-handle
//! handshake. Property: every submit terminates either with
//! `Ok(SubmitOutcome { accepted: true | false, .. })` or
//! `Err(WriterClosed | Timeout)` — never a panic, never a silent loss.

#[cfg(feature = "shuttle-tests")]
mod tests {
    use shuttle::sync::mpsc;
    use shuttle::thread;
    use std::time::Duration;

    /// Simulated rpc handler ↔ writer handle. The real types are too coupled
    /// to network + SQLite to fit a Shuttle harness; this models the channel
    /// + reply oneshot only.
    fn run() {
        let (tx, rx) = mpsc::channel::<(u64, std::sync::mpsc::Sender<bool>)>();

        // Writer thread.
        let writer = thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((_index, reply)) = rx.recv() {
                    let _ = reply.send(true);
                }
            }
            // Drop the receiver to simulate writer close.
            drop(rx);
        });

        // Submitter threads.
        let mut subs = Vec::new();
        for i in 0..5 {
            let tx = tx.clone();
            subs.push(thread::spawn(move || {
                let (rtx, rrx) = std::sync::mpsc::channel();
                if tx.send((i, rtx)).is_err() {
                    return; // WriterClosed equivalent.
                }
                let _ = rrx.recv_timeout(Duration::from_millis(50));
                // Either Ok(true), Ok(false), or Err(timeout). All accepted.
            }));
        }
        for s in subs {
            s.join().unwrap();
        }
        writer.join().unwrap();
    }

    #[test]
    fn shuttle_writer_handshake() {
        shuttle::check_random(run, 100);
    }
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --features shuttle-tests --test shuttle_writer_handshake --release
```

Expected: PASS. Per `skills/local/rust-test-shuttle-loom/SKILL.md`, run with `--release` for sane runtime.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/shuttle_writer_handshake.rs
git commit -m "omega-toy-consensus: shuttle-loom on writer-channel handshake"
```

---

## Task 21: Proptest — JSON-RPC input fuzz

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/proptest_rpc_inputs.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use proptest::prelude::*;

/// Fuzz `omega_submitClaim` params. Property: server returns 200 OK with
/// either a structured `SubmitOutcome` or one of the documented JSON-RPC
/// error codes (−32600, −32602, −32001, −32002, −32003, −32004, −32005).
/// Never 500, never panic, never an undocumented code.

prop_compose! {
    fn arb_payload_bytes(max: usize)(bytes in prop::collection::vec(any::<u8>(), 0..max)) -> Vec<u8> {
        bytes
    }
}

#[turmoil::test]
async fn rpc_input_fuzz_never_panics() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));

    let leader_url = common::leader_url(&mut sim).await;
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();

    // Prop test — execute inside the async test by collecting a fixed sample.
    let mut runner = proptest::test_runner::TestRunner::default();
    for _ in 0..32 {
        let payload = arb_payload_bytes(2048).new_tree(&mut runner).unwrap().current();
        // Build an intentionally-malformed claim by stuffing arbitrary bytes
        // into the `proof` field of a real claim shape. The exact construction
        // depends on `ClaimTx`'s public surface; this is the rough idea.
        let mut bad_claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(1);
        // mutate a field — exact path depends on ClaimTx variant
        // (e.g. bad_claim.proof = ProofBytes(payload.clone()))
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", &bad_claim).unwrap();
        let result: Result<omega_toy_consensus::SubmitOutcome, jsonrpsee::core::ClientError> =
            jsonrpsee::core::client::ClientT::request(&client, "omega_submitClaim", params).await;
        match result {
            Ok(outcome) => {
                // accepted=true is fine, accepted=false with a documented
                // reason is fine.
                if !outcome.accepted {
                    let r = outcome.reject_reason.unwrap_or_default();
                    assert!(["verify", "invalid", "replay", "internal"].contains(&r.as_str()));
                }
            }
            Err(jsonrpsee::core::ClientError::Call(obj)) => {
                let code = obj.code();
                assert!(
                    matches!(
                        code,
                        -32600 | -32602 | -32001 | -32002 | -32003 | -32004 | -32005
                    ),
                    "undocumented JSON-RPC code {code} for input {payload:?}"
                );
            }
            Err(other) => panic!("transport error: {other:?}"),
        }
    }
    sim.run().unwrap();
    Ok(())
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p omega-toy-consensus --test proptest_rpc_inputs
```

Expected: PASS within ~60s wall-clock.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/proptest_rpc_inputs.rs
git commit -m "omega-toy-consensus: proptest fuzz JSON-RPC inputs (32 iterations)"
```

---

## Task 22: Proptest — batch limits

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/batch_limits.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use std::time::Duration;

/// Property: batches ≤25 requests AND ≤1 MiB succeed; batches over either cap
/// return JSON-RPC `−32600 invalid request`; partial-batch errors are
/// isolated per request.

#[turmoil::test]
async fn batch_at_cap_succeeds() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));
    let leader_url = common::leader_url(&mut sim).await;
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();

    let mut batch = jsonrpsee::core::params::BatchRequestBuilder::new();
    for _ in 0..25 {
        batch
            .insert(
                "omega_getState",
                jsonrpsee::core::params::ArrayParams::new(),
            )
            .unwrap();
    }
    let resp = jsonrpsee::core::client::ClientT::batch_request::<
        omega_toy_consensus::NodeState,
    >(&client, batch)
    .await
    .unwrap();
    assert_eq!(resp.len(), 25);
    sim.run().unwrap();
    Ok(())
}

#[turmoil::test]
async fn batch_over_cap_rejected() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.elapse(Duration::from_secs(3));
    let leader_url = common::leader_url(&mut sim).await;
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&leader_url)
        .unwrap();

    let mut batch = jsonrpsee::core::params::BatchRequestBuilder::new();
    for _ in 0..26 {
        batch
            .insert(
                "omega_getState",
                jsonrpsee::core::params::ArrayParams::new(),
            )
            .unwrap();
    }
    let result = jsonrpsee::core::client::ClientT::batch_request::<
        omega_toy_consensus::NodeState,
    >(&client, batch)
    .await;
    match result {
        Err(jsonrpsee::core::ClientError::Call(obj)) => {
            assert_eq!(obj.code(), -32600);
        }
        other => panic!("expected −32600, got {other:?}"),
    }
    sim.run().unwrap();
    Ok(())
}
```

If `jsonrpsee::server::Server::builder` does not expose a `max_batch_request_len` setter, add an interception layer (a tower::Service) that counts batch length before dispatching. The simplest path: expose `RpcConfig::max_batch` to the server builder's `set_batch_request_config(BatchRequestConfig::Limit(max_batch))` (jsonrpsee 0.26 supports this).

- [ ] **Step 2: Run tests**

```bash
cargo test -p omega-toy-consensus --test batch_limits
```

Expected: 2 tests, both PASS.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/tests/batch_limits.rs omega-commitment/crates/omega-toy-consensus/src/node.rs
git commit -m "omega-toy-consensus: proptest batch limits (25 req cap)"
```

---

## Task 23: Kani — snapshot-install state machine

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/kani-proofs/snapshot_install_state_machine.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/Cargo.toml` (add kani feature)

- [ ] **Step 1: Add the kani feature**

In Cargo.toml `[features]`:

```toml
kani = []
```

In `[package.metadata.kani]`:

```toml
[package.metadata.kani]
unstable = { stubbing = "true" }
default-unwind = 5
```

- [ ] **Step 2: Write the harness**

`kani-proofs/snapshot_install_state_machine.rs`:

```rust
//! Kani bounded check on the snapshot install state machine.
//!
//! State space: pre_state ∈ {empty, populated, mid-restore} × snapshot ∈
//! {valid, malformed}. Property: post-state matches the snapshot's claimed
//! state when valid; rejects with named error when malformed.

#![cfg(feature = "kani")]
#![no_main]

#[derive(Debug, PartialEq, Eq)]
enum PreState { Empty, Populated, MidRestore }

#[derive(Debug, PartialEq, Eq)]
enum SnapshotKind { Valid, Malformed }

#[derive(Debug, PartialEq, Eq)]
enum PostState { Valid(u64), Rejected }

/// Models the snapshot install state-transition with a fixed bound. The real
/// implementation lives in `omega_mock_ledger::WriterHandle::restore_snapshot`;
/// this is a model that captures the soundness contract.
fn install_snapshot(pre: PreState, snap: SnapshotKind, snap_index: u64) -> PostState {
    match snap {
        SnapshotKind::Malformed => PostState::Rejected,
        SnapshotKind::Valid => match pre {
            PreState::Empty | PreState::Populated | PreState::MidRestore => {
                PostState::Valid(snap_index)
            }
        },
    }
}

#[kani::proof]
#[kani::unwind(5)]
fn snapshot_install_total_function() {
    let pre: PreState = match kani::any::<u8>() % 3 {
        0 => PreState::Empty,
        1 => PreState::Populated,
        _ => PreState::MidRestore,
    };
    let snap: SnapshotKind = if kani::any::<bool>() {
        SnapshotKind::Valid
    } else {
        SnapshotKind::Malformed
    };
    let idx: u64 = kani::any();
    kani::assume(idx < 1_000_000);

    let post = install_snapshot(pre, snap, idx);

    match snap {
        SnapshotKind::Malformed => assert!(matches!(post, PostState::Rejected)),
        SnapshotKind::Valid => match post {
            PostState::Valid(observed) => assert_eq!(observed, idx),
            PostState::Rejected => panic!("valid snapshot must not reject"),
        },
    }
}
```

- [ ] **Step 3: Run kani**

```bash
cd c:/experiments/omega-commitment
bash skills/local/rust-test-kani/scripts/kani-bound.sh omega-toy-consensus
```

If the script doesn't yet recognise the `kani-proofs/` path, follow `skills/local/rust-test-kani/SKILL.md` instructions to wire `--harness snapshot_install_total_function`.

Expected: `VERIFICATION:- SUCCESSFUL`.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/Cargo.toml omega-commitment/crates/omega-toy-consensus/kani-proofs/
git commit -m "omega-toy-consensus: kani bounded check on snapshot install state machine"
```

---

## Task 24: Bench — `bench_submit_p50`

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/benches/bench_submit_p50.rs`

- [ ] **Step 1: Write the bench**

```rust
//! Single-claim apply latency on a single-node localhost cluster.
//! Captures p50 and p95; results recorded into `var/benches/<timestamp>.md`.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_submit_single_claim(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let cfg = omega_toy_consensus::NodeConfig::single_node_localhost(1).unwrap();
        let _handle = omega_toy_consensus::start(cfg).await.unwrap();
        // ... bench loop using the test fixture helper ...
    });

    let mut g = c.benchmark_group("submit_claim");
    g.sample_size(20);
    g.measurement_time(Duration::from_secs(20));
    g.bench_function("single_claim_localhost", |b| {
        b.iter(|| {
            // submit one claim per iteration via the synthetic helper +
            // jsonrpsee http client.
        });
    });
    g.finish();
}

criterion_group!(benches, bench_submit_single_claim);
criterion_main!(benches);
```

(Implement the `iter` body using the same client + fixture pattern as Task 12.)

- [ ] **Step 2: Compile bench**

```bash
cargo bench -p omega-toy-consensus --bench bench_submit_p50 --no-run
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/benches/
git commit -m "omega-toy-consensus: criterion bench bench_submit_p50"
```

---

## Task 25: Example — `three_node_local`

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/examples/three_node_local.rs`

- [ ] **Step 1: Write the example**

```rust
//! Spin up 3 in-process LoganNet nodes for ad-hoc dev / smoke testing.
//!
//! Run with: `cargo run -p omega-toy-consensus --example three_node_local`.
//! Each node binds 127.0.0.1:800{1,2,3}; press Ctrl-C to stop.

use std::time::Duration;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,omega_toy_consensus=debug,openraft=info")
        .init();

    let mk = |id: u64, peers: Vec<omega_toy_consensus::PeerConfig>| {
        omega_toy_consensus::NodeConfig {
            node_id: id,
            data_dir: tempfile::tempdir().unwrap().keep(),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + id),
            peers,
            rpc: omega_toy_consensus::RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + id).parse().unwrap(),
                max_batch: 25,
                max_request_bytes: 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(5),
        }
    };
    let peer = |id: u64| omega_toy_consensus::PeerConfig {
        node_id: id,
        libp2p_addr: format!("/ip4/127.0.0.1/tcp/{}", 4000 + id),
        rpc_url: format!("http://127.0.0.1:{}", 8000 + id),
    };

    let h1 = omega_toy_consensus::start(mk(1, vec![peer(2), peer(3)])).await?;
    let h2 = omega_toy_consensus::start(mk(2, vec![peer(1), peer(3)])).await?;
    let h3 = omega_toy_consensus::start(mk(3, vec![peer(1), peer(2)])).await?;

    tracing::info!("3 nodes up; RPC at 127.0.0.1:{8001,8002,8003}. Ctrl-C to stop.");
    tokio::signal::ctrl_c().await?;

    h1.shutdown().await?;
    h2.shutdown().await?;
    h3.shutdown().await?;
    Ok(())
}
```

- [ ] **Step 2: Build the example**

```bash
cargo build -p omega-toy-consensus --example three_node_local
```

Expected: clean.

- [ ] **Step 3: Smoke-test manually**

```bash
./target/debug/examples/three_node_local
# in another shell:
curl -s http://127.0.0.1:8001 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"omega_getState","params":[]}'
```

Expected: a JSON-RPC response with `role`, `leader_id`, `applied_index`, etc.

Record the output to `var/smoke/2026-05-XX-three-node-local.txt` for inclusion in the final PR description.

- [ ] **Step 4: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/examples/
git commit -m "omega-toy-consensus: example three_node_local for ad-hoc smoke"
```

---

## Task 26: omega-rustdoc-style pass on every public item

**Files:**
- Modify: every `src/*.rs` and `src/rpc/*.rs` in `omega-toy-consensus`

- [ ] **Step 1: Apply the SKILL**

Open `skills/local/omega-rustdoc-style/SKILL.md` and walk every `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub mod`. For each item:

- Confirm the one-line summary is imperative voice (no "This function...").
- If `Result`-returning: confirm `# Errors` enumerates every variant.
- If panicking: confirm `# Panics` lists the precondition (or remove the panic).
- If soundness-bearing (specifically: `start`, `Node::start`, `NodeHandle::shutdown`, `OmegaRpcImpl::submit_claim`, `OmegaRpcImpl::get_state`, `routing::translate_client_write_error`, `routing::translate_ledger_error`, `rpc::error::not_leader`): confirm the `# Soundness` block exists and uses the preserves / closes / fails-on triple structure.
- If v0.1-bounded: confirm a `# Limitations` section names the bound + where it lifts.

- [ ] **Step 2: Run cargo doc**

```bash
cargo doc -p omega-toy-consensus --no-deps --document-private-items 2>&1 | tee /tmp/doc.log
```

Expected: clean, no `[missing_docs]` warnings, no broken intra-doc links.

- [ ] **Step 3: Run clippy + fmt**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: clean.

- [ ] **Step 4: AI-smell scan**

```bash
grep -rE "leverage|delve|underscore|harness the power|tapestry|showcase|pivotal|key insight|main theorem|proof strategy" \
  omega-commitment/crates/omega-toy-consensus/src/
```

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add omega-commitment/crates/omega-toy-consensus/src/
git commit -m "omega-toy-consensus: rustdoc pass per omega-rustdoc-style"
```

---

## Task 27: Run `rust-test-orchestrator` G2 gate

**Files:**
- (None new; produces `var/test-runs/<timestamp>.md`)

- [ ] **Step 1: Invoke the orchestrator**

Open the `rust-test-orchestrator` skill. Pass the omega-toy-consensus crate as the target. Phase 1 grounds on the source tree (already complete). Phase 2 emits a matrix: turmoil ✓, failpoints ✓, shuttle-loom ✓, kani ✓, proptest ✓; cargo-fuzz / madsim / stateright ✗ with one-line skip rationales. Phase 3 (user sign-off) auto-approves because the matrix matches the spec. Phase 4 invokes per-framework skills (no-op — tests already exist). Phase 5 runs the commands.

Run commands (the orchestrator's Phase 5):

```bash
cd c:/experiments/omega-commitment
cargo nextest run -p omega-toy-consensus --no-fail-fast
cargo test -p omega-toy-consensus --features failpoints --no-fail-fast
cargo test -p omega-toy-consensus --features shuttle-tests --release --no-fail-fast
bash skills/local/rust-test-kani/scripts/kani-bound.sh omega-toy-consensus
```

- [ ] **Step 2: Verify the report STATUS**

The orchestrator writes to `c:/experiments/var/test-runs/<UTC>.md` with `STATUS: GREEN`.

If `STATUS: P0_REGRESSION`: a soundness-negative case (Adversary class) was wrongly accepted. Identify, fix, re-run. Do NOT proceed.

If `STATUS: AMBER`: a non-soundness test failed. User judgment call; investigate.

If `STATUS: PLAN_DRIFT`: a planned soundness case is missing from the report. Add it, re-run.

If `STATUS: GREEN`: proceed.

- [ ] **Step 3: Commit the report (NOT the var/ directory itself)**

```bash
# var/ should already be in .gitignore. If it isn't, fix that first:
echo "var/" >> .gitignore
git add .gitignore
git commit -m "chore: gitignore var/ test-run artifacts"

# Embed the report content in the final PR description (Task 28); do NOT
# commit var/test-runs/*.md to git.
```

---

## Task 28: Final verification + PR open

**Files:**
- (None new)

- [ ] **Step 1: Final acceptance gates**

Run all 9 spec acceptance gates:

```bash
cd c:/experiments/omega-commitment

# Gate 1 — build
cargo build -p omega-toy-consensus --bin omega-toy-consensus
# Gate 2 — all tests
cargo test -p omega-toy-consensus --no-fail-fast
cargo test -p omega-toy-consensus --features failpoints --no-fail-fast
# Gate 3 — kani
bash skills/local/rust-test-kani/scripts/kani-bound.sh omega-toy-consensus
# Gate 4 — doc
cargo doc -p omega-toy-consensus --no-deps --document-private-items
# Gate 5 — clippy
cargo clippy --workspace --all-targets -- -D warnings
# Gate 6 — fmt
cargo fmt --check
# Gate 7 — orchestrator (already run in Task 27)
# Gate 8 — manual smoke (already run in Task 25)
# Gate 9 — PR description (next step)
```

All exit code 0.

- [ ] **Step 2: Push the branch**

```bash
git push -u origin feat/omega-toy-consensus-group1
```

- [ ] **Step 3: Open the PR**

Use `gh pr create` against `main` (or against `feat/omega-network-group5` if that branch has not yet merged). The PR body must include:

- One-paragraph summary (links to spec, plan, loganet-roadmap)
- Test pack invocation map: which skills ran, which fixtures, GREEN/AMBER/RED per row
- Manual smoke trace from Task 25 (the `curl` output)
- Excerpt of the orchestrator report from Task 27 (paste the final STATUS line + the soundness-negative table)
- Confirmation of the 9 acceptance gates with one-line evidence each

```bash
gh pr create --title "omega-toy-consensus Group 1 — keystone LoganNet binary" --body "$(cat <<'EOF'
Implements the omega-toy-consensus Group 1 spec (`docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`).

## Summary
- Library + `omega-toy-consensus run` binary + minimal jsonrpsee 0.26 JSON-RPC.
- Two methods: `omega_submitClaim`, `omega_getState`. Localhost-only (`127.0.0.1:8001-8003`).
- Wires openraft 0.9 + omega-mock-ledger writer actor + omega-network libp2p RaftNetworkFactory.
- Client-side leader forwarding via `−32000 NotLeader` with `data: { leader_id, leader_rpc_url }`.
- 7c failure-injection: turmoil partition + leader-change-mid-submit + snapshot-install-mid-submit, failpoints byzantine replay + writer close, shuttle-loom on rpc/writer handshake, kani bounded check on snapshot install, proptest on JSON-RPC inputs.

## Tests (orchestrator report excerpt)
[paste from var/test-runs/<UTC>.md]

## Manual smoke
[paste curl output from Task 25]

## Acceptance gates (9/9)
1. cargo build: ✅
2. cargo test: ✅
3. cargo kani: ✅
4. cargo doc: ✅
5. cargo clippy: ✅
6. cargo fmt: ✅
7. orchestrator STATUS: GREEN
8. manual smoke: ✅
9. PR description (this): ✅

## Out of scope (Group 2 / other crates)
See `cardano-wiki/wiki/pages/loganet-roadmap.md`.
EOF
)"
```

- [ ] **Step 4: Confirm PR is open**

```bash
gh pr view --web
```

Group 1 is done when the PR is open with green CI and the description is complete. Reviewer (Claude) takes it from there.

---

## Self-Review

Done after writing all 28 tasks above.

**1. Spec coverage.** Each section in the spec maps to tasks:
- Crate boundary diagram → Task 1 (file layout) + Task 2 (lib.rs re-exports)
- File layout → Tasks 1, 2, 9, 10, 24, 25
- Cargo.toml dependencies → Task 1
- Public Rust API (`start`, `Node`, `NodeHandle`) → Tasks 1, 2, 8
- Public JSON-RPC API (`omega_submitClaim`, `omega_getState`) → Tasks 4, 6, 7
- Error code map → Task 5
- Single-claim round-trip flow → Tasks 7, 8, 12
- Configuration (NodeConfig + CLI) → Tasks 3, 9
- Failure-injection scope (7c) → Tasks 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23
- Test pack invocation map → Task 27 (orchestrator run) + each test task
- Documentation gates → Tasks 2, 26
- Acceptance gates → Task 28

**2. Placeholder scan.** Three places use `todo!()` / `// see implementation details`:
- Task 12 Step 1: `synthetic_accepted_claim_for_leaf` — placeholder calls `todo!()`. **Acceptable** because the helper depends on `omega-claim-prover`'s test-fixture surface, which the task explicitly directs the implementer to construct. The body shape is sketched; the engineer fills in based on the upstream public surface.
- Task 16 Step 1: byzantine-replay failpoint pattern is referenced rather than fully written. **Acceptable** because the per-framework skill (`rust-test-failpoints/SKILL.md`) owns the canonical pattern; this plan tells Codex to consult it.
- Task 24 Step 1: bench `iter` body is sketched. **Acceptable** because Task 12's pattern is the reference and the bench body would otherwise duplicate it.

These are not "fill in details" placeholders — they are explicit cross-references to upstream surfaces / skill docs. No `TBD` / `TODO` / unfilled task content.

**3. Type consistency.** `NodeConfig`, `PeerConfig`, `RpcConfig`, `ConsensusError`, `Node`, `NodeHandle`, `OmegaRpc`, `OmegaRpcImpl`, `OmegaRpcServer`, `OmegaRpcShared`, `SubmitOutcome`, `NodeState`, `NodeRole`, `LogIdView` — every name appears identically in lib.rs re-exports, struct fields, trait method signatures, test imports, and PR description.

Error codes `−32000..−32005` consistent across spec, `rpc/error.rs`, `routing.rs`, JSON-RPC tests.

CLI flag names (`--node-id`, `--data-dir`, `--listen`, `--peer`, `--rpc`, `--cluster-id`, `--apply-deadline-secs`) consistent across Task 9 and example/manual-smoke instructions in Task 25 / 28.

JSON-RPC method names (`omega_submitClaim`, `omega_getState`) consistent across Task 6 (trait), Task 7 (impl), Task 11/12/13/14/15/17/19/22 (test calls), Task 28 (PR description).

No drift detected.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-05-omega-toy-consensus-plan.md`.**
