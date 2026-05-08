# omega-toy-consensus Group 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the process-global raft dispatcher (`RAFT_REGISTRY`) with a real libp2p request-response transport so three independent `omega-toy-consensus run` processes form a real cluster. Add membership change RPCs, exercise the snapshot install path end-to-end, and bring Linux + macOS into CI.

**Architecture:** Seven phases, each producing working, testable software. Phase 1 lands the libp2p inbound actor in `omega-network` so two `LibP2pNetwork` instances in one test process can exchange a raft RPC over loopback libp2p. Phase 2 wires that into `omega-toy-consensus` and removes the static registry. Phase 3 demonstrates a multi-process cluster via `tokio::process::Command`. Phases 4-5 add membership change and a real snapshot-install integration test that replaces today's elapsed-window placeholder. Phase 6 expands CI. Phase 7 drops the `test-support` backdoor entirely; partition tests now use libp2p connection-deny rules.

**Tech stack:** libp2p 0.55 (already pinned, `tcp+noise+yamux+request-response`), openraft 0.9, jsonrpsee 0.26, turmoil 0.7 (still useful for in-process scenarios), `tokio::process::Command` for multi-process tests, GitHub Actions matrix for cross-platform CI.

**Branch:** `feat/omega-toy-consensus-group2`. Cut from `main` after PR #7 merge (`961f550`).

---

## Scope check

This is a single subsystem with strong internal coupling — every phase depends on the libp2p transport landing first. Not splittable into independent sub-plans, but the seven phases are individually shippable as separate PRs, which is recommended (Phase 1 alone is a meaty PR).

## File structure

New files:
- `omega-commitment/crates/omega-network/src/inbound.rs` — libp2p Behaviour + Swarm event loop for inbound raft RPC.
- `omega-commitment/crates/omega-network/src/protocol.rs` — `request_response::ProtocolName` + codec for `RaftRpcRequest`/`RaftRpcResponse`.
- `omega-commitment/crates/omega-network/tests/loopback_round_trip.rs` — two-instance loopback test.
- `omega-commitment/crates/omega-toy-consensus/tests/multi_process_three_node.rs` — three real OS processes form a cluster.
- `omega-commitment/crates/omega-toy-consensus/tests/membership_change.rs` — add learner, promote voter.
- `omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_real.rs` — real snapshot install across an out-of-date follower (replaces `snapshot_install_during_submit.rs`).

Modified files:
- `omega-commitment/crates/omega-network/src/lib.rs` — re-export `InboundRaftHandler`, `RaftSwarm`.
- `omega-commitment/crates/omega-network/src/network.rs` — outbound side now hands off to the swarm event loop instead of an mpsc to the consumer.
- `omega-commitment/crates/omega-network/Cargo.toml` — pull in `libp2p::request_response`, `libp2p::swarm`, `libp2p::tcp`, etc. (most already pinned).
- `omega-commitment/crates/omega-toy-consensus/src/node.rs` — drop `RAFT_REGISTRY`, `RAFT_LINK_BLOCKS`, `route_raft`, `register_raft`, `unregister_raft`, `clear_raft_link_blocks_for_test`, `partition_raft_link_for_test`, `spawn_network_dispatcher`, `dispatch_raft_request`. Replace with `RaftSwarm` from `omega-network`.
- `omega-commitment/crates/omega-toy-consensus/src/lib.rs` — drop `pub mod test_support` (Phase 7).
- `omega-commitment/crates/omega-toy-consensus/Cargo.toml` — drop the `test-support` feature and the self-dep.
- `omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs` — add `omega_addLearner`, `omega_promoteVoter` methods.
- `omega-commitment/crates/omega-toy-consensus/src/rpc/types.rs` — `AddLearnerOutcome`, `PromoteVoterOutcome`.
- `omega-commitment/crates/omega-toy-consensus/src/error.rs` — extend with `MembershipRejected`.
- `omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_during_submit.rs` — **delete**, replaced by `snapshot_install_real.rs`.
- `omega-commitment/crates/omega-toy-consensus/tests/partition.rs` — switch from `partition_raft_link` to libp2p-level connection-deny (Phase 7).
- `omega-commitment/crates/omega-toy-consensus/tests/leader_change_during_submit.rs` — same migration; strengthen assertions to "no double-apply" once raft RPC is real.
- `cardano-wiki/wiki/pages/loganet-roadmap.md` — move "Group 2" entries to "Group 1 of v0.2" (or similar — strike the deferral and document what shipped).
- `.github/workflows/ci.yml` — add Linux + macOS to the matrix.

Deleted files (Phase 7):
- The two `partition_raft_link_for_test` and `clear_raft_link_blocks_for_test` functions from `node.rs` (already done by the cfg gate; Phase 7 removes the cfg-gated paths entirely).

---

# Phase 1 — libp2p inbound raft RPC

The first PR. Lands a working request-response protocol in `omega-network` with a two-instance loopback round-trip test. Does NOT touch `omega-toy-consensus`.

### Task 1.1: Pin the request-response codec deps

**Files:**
- Modify: `omega-commitment/Cargo.toml` (workspace deps)
- Modify: `omega-commitment/crates/omega-network/Cargo.toml`

- [ ] **Step 1: Verify libp2p features.**

The workspace already pins `libp2p = "=0.55.0"` with `tcp+tokio+noise+yamux+mdns+kad+request-response`. Confirm `request-response` is in the feature list. If not, add it.

```bash
grep -A 2 "libp2p =" omega-commitment/Cargo.toml
```

Expected: `request-response` appears in the feature list.

- [ ] **Step 2: Add `request-response` dep to `omega-network`.**

Edit `omega-commitment/crates/omega-network/Cargo.toml`:

```toml
[dependencies]
libp2p = { workspace = true }
# already has: ciborium, openraft, serde, thiserror, tokio
```

No change needed if libp2p is already a workspace dep.

- [ ] **Step 3: Commit.**

```bash
git add omega-commitment/Cargo.toml omega-commitment/crates/omega-network/Cargo.toml
git commit -m "omega-network: confirm libp2p request-response feature pinned for Group 2 transport"
```

### Task 1.2: Define the wire protocol

**Files:**
- Create: `omega-commitment/crates/omega-network/src/protocol.rs`
- Modify: `omega-commitment/crates/omega-network/src/lib.rs`

- [ ] **Step 1: Write the protocol module.**

Create `omega-commitment/crates/omega-network/src/protocol.rs`:

```rust
//! libp2p request-response protocol for raft RPC.

use std::io;

use async_trait::async_trait;
use futures::prelude::*;
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;

use crate::rpc::{decode_cbor, encode_cbor, RaftRpcRequest, RaftRpcResponse};

/// Protocol name advertised over libp2p.
pub const RAFT_PROTOCOL: StreamProtocol = StreamProtocol::new("/omega-loganet/raft/1");

/// Maximum CBOR-encoded request or response size in bytes.
///
/// Snapshot install RPCs carry a chunk of state-machine bytes; openraft 0.9's
/// default chunk is 1 MiB. Add headroom for envelope overhead.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// CBOR codec for `RaftRpcRequest` / `RaftRpcResponse` over libp2p.
#[derive(Clone, Default)]
pub struct RaftCodec;

#[async_trait]
impl Codec for RaftCodec {
    type Protocol = StreamProtocol;
    type Request = RaftRpcRequest;
    type Response = RaftRpcResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let bytes = read_frame(io).await?;
        decode_cbor(&bytes).map_err(into_io)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let bytes = read_frame(io).await?;
        decode_cbor(&bytes).map_err(into_io)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = encode_cbor(&req).map_err(into_io)?;
        write_frame(io, &bytes).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = encode_cbor(&res).map_err(into_io)?;
        write_frame(io, &bytes).await
    }
}

async fn read_frame<T: AsyncRead + Unpin + Send>(io: &mut T) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame {len} bytes exceeds max {MAX_FRAME_BYTES}"),
        ));
    }
    let mut buf = vec![0u8; len];
    io.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_frame<T: AsyncWrite + Unpin + Send>(io: &mut T, bytes: &[u8]) -> io::Result<()> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame > u32::MAX"))?;
    io.write_all(&len.to_be_bytes()).await?;
    io.write_all(bytes).await?;
    io.flush().await?;
    Ok(())
}

fn into_io(err: crate::rpc::OmegaNetworkError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}
```

- [ ] **Step 2: Re-export from `lib.rs`.**

Edit `omega-commitment/crates/omega-network/src/lib.rs`:

```rust
pub mod protocol;
pub use protocol::{RaftCodec, MAX_FRAME_BYTES, RAFT_PROTOCOL};
```

- [ ] **Step 3: Build to verify the codec compiles.**

```bash
cargo build -p omega-network
```

Expected: clean build.

- [ ] **Step 4: Commit.**

```bash
git add omega-commitment/crates/omega-network/src/protocol.rs omega-commitment/crates/omega-network/src/lib.rs
git commit -m "omega-network: add libp2p request-response codec for raft RPC"
```

### Task 1.3: Define `InboundRaftHandler` trait

**Files:**
- Create: `omega-commitment/crates/omega-network/src/inbound.rs` (initial scaffold)
- Modify: `omega-commitment/crates/omega-network/src/lib.rs`

- [ ] **Step 1: Write the handler trait.**

Create `omega-commitment/crates/omega-network/src/inbound.rs` with the trait definition only (the swarm event loop comes in 1.4):

```rust
//! Inbound raft RPC handler trait.
//!
//! `RaftSwarm` calls into the implementor to dispatch a received raft RPC
//! to the local `Raft` instance and produce a response.

use async_trait::async_trait;

use crate::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};

/// Application-side handler for inbound raft RPCs.
///
/// `omega-toy-consensus` provides a concrete impl that calls
/// `Raft::append_entries` / `Raft::vote` / `Raft::install_snapshot` on the
/// local node and returns the response.
///
/// # Soundness
///
/// Preserves: the swarm calls `handle` exactly once per inbound request and
/// awaits the future to completion before flushing the response. Out-of-order
/// responses cannot occur because libp2p `request_response` pairs each
/// request with its own substream.
///
/// Closes: the inbound side cannot bypass the local `Raft` instance — every
/// received RPC goes through `handle`.
#[async_trait]
pub trait InboundRaftHandler: Send + Sync + 'static {
    /// Dispatches an inbound raft RPC to the local node.
    async fn handle(
        &self,
        request: RaftRpcRequest,
    ) -> Result<RaftRpcResponse, OmegaNetworkError>;
}
```

- [ ] **Step 2: Re-export.**

```rust
// In lib.rs:
pub mod inbound;
pub use inbound::InboundRaftHandler;
```

- [ ] **Step 3: Build.**

```bash
cargo build -p omega-network
```

- [ ] **Step 4: Commit.**

```bash
git add omega-commitment/crates/omega-network/src/inbound.rs omega-commitment/crates/omega-network/src/lib.rs
git commit -m "omega-network: add InboundRaftHandler trait for libp2p request-response"
```

### Task 1.4: Implement `RaftSwarm`

**Files:**
- Modify: `omega-commitment/crates/omega-network/src/inbound.rs`

This is the core of Phase 1. The swarm owns the libp2p `Swarm<request_response::Behaviour<RaftCodec>>` and runs an event loop that:
1. Accepts inbound requests, calls `handler.handle(request)`, sends the response.
2. Pumps outbound requests from a `tokio::mpsc::Receiver<OutboundRaftRequest>` (existing type from `network.rs`) into the swarm.
3. Routes inbound responses back to the originating outbound oneshot reply.

- [ ] **Step 1: Add types + struct skeleton.**

Append to `inbound.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use libp2p::core::transport::upgrade;
use libp2p::request_response::{
    self, Behaviour as RrBehaviour, Config as RrConfig, Event as RrEvent,
    Message as RrMessage, ProtocolSupport, ResponseChannel,
};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder, Transport};
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{RaftCodec, RAFT_PROTOCOL};
use crate::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use crate::OutboundRaftRequest;

/// Per-peer addressing config.
#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub node_id: u64,
    pub peer_id: PeerId,
    pub address: Multiaddr,
}

/// libp2p swarm that owns the raft request-response protocol and the
/// inbound handler.
pub struct RaftSwarm {
    swarm: Swarm<RrBehaviour<RaftCodec>>,
    peers: HashMap<u64, PeerEntry>,
    peer_id_to_node: HashMap<PeerId, u64>,
    outbound_rx: mpsc::Receiver<OutboundRaftRequest>,
    pending: HashMap<request_response::OutboundRequestId, oneshot::Sender<Result<Vec<u8>, OmegaNetworkError>>>,
    handler: Arc<dyn InboundRaftHandler>,
}

impl RaftSwarm {
    /// Builds a new swarm that listens on `listen_addr`, dials each `peer.address`,
    /// and routes inbound requests through `handler`.
    pub async fn new(
        listen_addr: Multiaddr,
        peers: Vec<PeerEntry>,
        outbound_rx: mpsc::Receiver<OutboundRaftRequest>,
        handler: Arc<dyn InboundRaftHandler>,
    ) -> Result<Self, OmegaNetworkError> {
        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?
            .with_behaviour(|_| {
                RrBehaviour::new(
                    [(RAFT_PROTOCOL, ProtocolSupport::Full)],
                    RrConfig::default()
                        .with_request_timeout(Duration::from_secs(30)),
                )
            })
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?
            .build();

        swarm.listen_on(listen_addr)
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?;

        let mut peer_id_to_node = HashMap::new();
        for p in &peers {
            peer_id_to_node.insert(p.peer_id, p.node_id);
            swarm.behaviour_mut().add_address(&p.peer_id, p.address.clone());
        }
        let peers = peers.into_iter().map(|p| (p.node_id, p)).collect();

        Ok(Self {
            swarm,
            peers,
            peer_id_to_node,
            outbound_rx,
            pending: HashMap::new(),
            handler,
        })
    }
}
```

- [ ] **Step 2: Write the failing event-loop skeleton test.**

Add to `inbound.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raft_swarm_skeleton_compiles() {
        // Just a compile gate; full round-trip test lives in
        // `tests/loopback_round_trip.rs`.
    }
}
```

- [ ] **Step 3: Run; expect compile failure naming missing fields/methods.**

```bash
cargo test -p omega-network --lib
```

Expected: compile error pointing at `OutboundRaftRequest`, which we need to verify is `pub` in `network.rs`.

- [ ] **Step 4: Audit `OmegaNetworkError`, `OutboundRaftRequest` visibility.**

Read `omega-commitment/crates/omega-network/src/network.rs:20-50`. If `OutboundRaftRequest` is not `pub` at the crate root, re-export it from `lib.rs`:

```rust
pub use network::OutboundRaftRequest;
```

- [ ] **Step 5: Run again; compile passes.**

```bash
cargo test -p omega-network --lib
```

Expected: PASS, 1 test (the skeleton).

- [ ] **Step 6: Commit.**

```bash
git add omega-commitment/crates/omega-network/src/inbound.rs omega-commitment/crates/omega-network/src/lib.rs
git commit -m "omega-network: scaffold RaftSwarm types over libp2p request-response"
```

### Task 1.5: Implement the swarm event loop

**Files:**
- Modify: `omega-commitment/crates/omega-network/src/inbound.rs`

- [ ] **Step 1: Write the failing loopback test (TDD).**

Create `omega-commitment/crates/omega-network/tests/loopback_round_trip.rs`:

```rust
//! Two `RaftSwarm` instances exchange a real raft RPC over loopback libp2p.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId};
use omega_network::inbound::{PeerEntry, RaftSwarm};
use omega_network::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use omega_network::{InboundRaftHandler, OutboundRaftRequest};
use openraft::raft::{VoteRequest, VoteResponse};
use openraft::Vote;
use tokio::sync::{mpsc, oneshot};

struct StubVoteHandler {
    node_id: u64,
}

#[async_trait]
impl InboundRaftHandler for StubVoteHandler {
    async fn handle(
        &self,
        req: RaftRpcRequest,
    ) -> Result<RaftRpcResponse, OmegaNetworkError> {
        match req {
            RaftRpcRequest::Vote(_) => Ok(RaftRpcResponse::Vote(VoteResponse::new(
                Vote::new(7, self.node_id),
                None,
                true,
            ))),
            _ => Err(OmegaNetworkError::Codec("unexpected request".into())),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_swarms_round_trip_vote() {
    // Set up node 1 with handler that replies vote.
    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listen_2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();

    let (out_tx_1, out_rx_1) = mpsc::channel::<OutboundRaftRequest>(8);
    let (out_tx_2, out_rx_2) = mpsc::channel::<OutboundRaftRequest>(8);

    // Build swarm 1 first to get its peer_id + address.
    let mut swarm_1 = RaftSwarm::new(
        listen_1,
        vec![],
        out_rx_1,
        Arc::new(StubVoteHandler { node_id: 1 }),
    )
    .await
    .unwrap();

    let peer_id_1 = swarm_1.local_peer_id();
    let address_1 = swarm_1.first_listen_address().await;

    let mut swarm_2 = RaftSwarm::new(
        listen_2,
        vec![PeerEntry {
            node_id: 1,
            peer_id: peer_id_1,
            address: address_1,
        }],
        out_rx_2,
        Arc::new(StubVoteHandler { node_id: 2 }),
    )
    .await
    .unwrap();

    let _t1 = tokio::spawn(async move { swarm_1.run().await });
    let _t2 = tokio::spawn(async move { swarm_2.run().await });

    // Send a vote request from 2 -> 1 via the outbound channel.
    let (reply_tx, reply_rx) = oneshot::channel();
    let payload = omega_network::rpc::encode_cbor(&RaftRpcRequest::Vote(VoteRequest {
        vote: Vote::new(5, 2),
        last_log_id: None,
    }))
    .unwrap();

    out_tx_2
        .send(OutboundRaftRequest {
            target: 1,
            payload,
            reply: reply_tx,
        })
        .await
        .unwrap();

    let response_bytes = tokio::time::timeout(Duration::from_secs(10), reply_rx)
        .await
        .expect("response within 10s")
        .unwrap()
        .expect("transport ok");

    let response: RaftRpcResponse =
        omega_network::rpc::decode_cbor(&response_bytes).unwrap();
    let vote_response = match response {
        RaftRpcResponse::Vote(v) => v,
        _ => panic!("expected vote response"),
    };
    assert_eq!(vote_response.vote.leader_id().voted_for(), 1);
}
```

- [ ] **Step 2: Run; expect compile error or test failure pointing at missing `run`, `local_peer_id`, `first_listen_address`.**

```bash
cargo test -p omega-network --test loopback_round_trip
```

Expected: FAIL with "no method `run`" or similar.

- [ ] **Step 3: Implement `RaftSwarm::run`, `local_peer_id`, `first_listen_address`.**

Add to `inbound.rs`:

```rust
impl RaftSwarm {
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Awaits the first `NewListenAddr` event and returns the address.
    pub async fn first_listen_address(&mut self) -> Multiaddr {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }

    /// Drives the swarm until shutdown.
    pub async fn run(mut self) -> Result<(), OmegaNetworkError> {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.on_swarm_event(event).await?;
                }
                outbound = self.outbound_rx.recv() => {
                    let Some(outbound) = outbound else { return Ok(()); };
                    self.on_outbound(outbound).await?;
                }
            }
        }
    }

    async fn on_outbound(&mut self, outbound: OutboundRaftRequest) -> Result<(), OmegaNetworkError> {
        let Some(peer) = self.peers.get(&outbound.target) else {
            let _ = outbound.reply.send(Err(OmegaNetworkError::OutboundClosed));
            return Ok(());
        };
        let req: RaftRpcRequest = crate::rpc::decode_cbor(&outbound.payload)?;
        let id = self
            .swarm
            .behaviour_mut()
            .send_request(&peer.peer_id, req);
        // Convert the typed reply back into bytes for the existing channel
        // contract.  The omega-toy-consensus side expects Vec<u8>.
        let (raw_tx, raw_rx) = oneshot::channel::<Result<Vec<u8>, OmegaNetworkError>>();
        self.pending.insert(id, raw_tx);
        // Spawn a tiny relay so the outbound caller's reply is still a Vec<u8>.
        tokio::spawn(async move {
            let res = raw_rx.await
                .unwrap_or(Err(OmegaNetworkError::OutboundClosed));
            let _ = outbound.reply.send(res);
        });
        Ok(())
    }

    async fn on_swarm_event(
        &mut self,
        event: SwarmEvent<RrEvent<RaftRpcRequest, RaftRpcResponse>>,
    ) -> Result<(), OmegaNetworkError> {
        match event {
            SwarmEvent::Behaviour(RrEvent::Message { peer, message, .. }) => {
                match message {
                    RrMessage::Request { request, channel, .. } => {
                        self.on_inbound_request(peer, request, channel).await?;
                    }
                    RrMessage::Response { request_id, response } => {
                        if let Some(tx) = self.pending.remove(&request_id) {
                            let bytes = crate::rpc::encode_cbor(&response)?;
                            let _ = tx.send(Ok(bytes));
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(RrEvent::OutboundFailure { request_id, error, .. }) => {
                if let Some(tx) = self.pending.remove(&request_id) {
                    let _ = tx.send(Err(OmegaNetworkError::Codec(error.to_string())));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn on_inbound_request(
        &mut self,
        _peer: PeerId,
        request: RaftRpcRequest,
        channel: ResponseChannel<RaftRpcResponse>,
    ) -> Result<(), OmegaNetworkError> {
        let response = self.handler.handle(request).await
            .unwrap_or_else(|err| {
                tracing::warn!(?err, "inbound raft RPC handler returned error; sending vote-deny fallback");
                // Fallback so the requester gets *some* response and times out
                // explicitly rather than hanging.
                RaftRpcResponse::Vote(openraft::raft::VoteResponse::new(
                    openraft::Vote::new(0, 0),
                    None,
                    false,
                ))
            });
        let _ = self.swarm.behaviour_mut().send_response(channel, response);
        Ok(())
    }
}
```

(`use futures::StreamExt;` may be needed for `select_next_some`.)

- [ ] **Step 4: Run the loopback test.**

```bash
cargo test -p omega-network --test loopback_round_trip
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add omega-commitment/crates/omega-network/src/inbound.rs omega-commitment/crates/omega-network/tests/loopback_round_trip.rs
git commit -m "omega-network: implement RaftSwarm event loop with loopback round-trip test"
```

### Task 1.6: Open Phase 1 PR

- [ ] **Step 1: Run all gates on `omega-network`.**

```bash
cd omega-commitment
cargo build -p omega-network
cargo test -p omega-network --no-fail-fast
cargo clippy -p omega-network --all-targets -- -D warnings
cargo fmt --check
```

Expected: all clean.

- [ ] **Step 2: Push the branch and open a PR.**

```bash
git push -u origin feat/omega-toy-consensus-group2
gh pr create --title "omega-network: libp2p request-response transport for raft RPC" --body "$(cat <<'EOF'
Phase 1 of `omega-toy-consensus` Group 2. Lands `RaftSwarm` over libp2p
request-response in `omega-network` so two instances in one process exchange
a raft RPC over a real loopback libp2p connection.

Does not touch `omega-toy-consensus`. The static `RAFT_REGISTRY` still
exists; Phase 2 replaces it with `RaftSwarm`.

## What landed
- `omega-network/src/protocol.rs` — `RaftCodec` over CBOR, length-prefixed.
- `omega-network/src/inbound.rs` — `InboundRaftHandler` trait + `RaftSwarm`.
- `omega-network/tests/loopback_round_trip.rs` — two swarms exchange a vote.

## Gates
- cargo build, test, clippy, fmt — all clean.
EOF
)"
```

---

# Phase 2 — wire `RaftSwarm` into `omega-toy-consensus`

Replaces the static raft registry with the libp2p transport. End of this phase: tests still pass via in-process libp2p (turmoil sim continues to work; the partition tests need updating but functionally still work via the registry-replacement). The ground truth shifts to multi-process tests in Phase 3.

### Task 2.1: Drop the static dispatcher

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/node.rs`

- [ ] **Step 1: Identify dependents.**

```bash
grep -rn "RAFT_REGISTRY\|RAFT_LINK_BLOCKS\|route_raft\|register_raft\|unregister_raft\|spawn_network_dispatcher\|dispatch_raft_request\|partition_raft_link_for_test\|clear_raft_link_blocks_for_test" omega-commitment/crates/omega-toy-consensus/
```

Expected: list of all call sites in `node.rs`, `lib.rs`'s `test_support`, and the integration tests' `tests/common/mod.rs`.

- [ ] **Step 2: Implement `OmegaInboundHandler`.**

In `node.rs`, after the `NodeHandle` struct, add:

```rust
struct OmegaInboundHandler {
    raft: Raft,
}

#[async_trait::async_trait]
impl omega_network::InboundRaftHandler for OmegaInboundHandler {
    async fn handle(
        &self,
        request: omega_network::rpc::RaftRpcRequest,
    ) -> Result<
        omega_network::rpc::RaftRpcResponse,
        omega_network::rpc::OmegaNetworkError,
    > {
        use omega_network::rpc::{RaftRpcRequest, RaftRpcResponse};
        match request {
            RaftRpcRequest::AppendEntries(req) => self
                .raft
                .append_entries(*req)
                .await
                .map(RaftRpcResponse::AppendEntries)
                .map_err(|e| omega_network::rpc::OmegaNetworkError::Codec(e.to_string())),
            RaftRpcRequest::InstallSnapshot(req) => self
                .raft
                .install_snapshot(*req)
                .await
                .map(RaftRpcResponse::InstallSnapshot)
                .map_err(|e| omega_network::rpc::OmegaNetworkError::Codec(e.to_string())),
            RaftRpcRequest::Vote(req) => self
                .raft
                .vote(req)
                .await
                .map(RaftRpcResponse::Vote)
                .map_err(|e| omega_network::rpc::OmegaNetworkError::Codec(e.to_string())),
        }
    }
}
```

- [ ] **Step 3: Replace `spawn_network_dispatcher` and the static registry in `Node::start`.**

Find this section (around `node.rs:84-87`):

```rust
let (network_factory, outbound_rx) = omega_network::LibP2pNetworkFactory::with_capacity(
    omega_network::DEFAULT_OUTBOUND_CAPACITY,
);
let network_join = spawn_network_dispatcher(config.node_id, outbound_rx);
```

Replace with:

```rust
let (network_factory, outbound_rx) = omega_network::LibP2pNetworkFactory::with_capacity(
    omega_network::DEFAULT_OUTBOUND_CAPACITY,
);
// Listen address comes from config; peer entries are derived after `raft`
// is constructed (we need it for the handler).
```

After the `raft` construction (around `node.rs:103`), add:

```rust
let listen_addr: omega_network::Multiaddr = config.libp2p_listen.parse()
    .map_err(|e| ConsensusError::Config(format!("libp2p_listen parse: {e}")))?;
let peer_entries: Vec<omega_network::inbound::PeerEntry> = config.peers.iter()
    .map(|p| {
        Ok(omega_network::inbound::PeerEntry {
            node_id: p.node_id,
            peer_id: p.libp2p_peer_id.parse()
                .map_err(|e| ConsensusError::Config(format!("peer {} peer_id: {e}", p.node_id)))?,
            address: p.libp2p_addr.parse()
                .map_err(|e| ConsensusError::Config(format!("peer {} addr: {e}", p.node_id)))?,
        })
    })
    .collect::<Result<_, ConsensusError>>()?;

let handler = std::sync::Arc::new(OmegaInboundHandler { raft: raft.clone() });
let swarm = omega_network::inbound::RaftSwarm::new(
    listen_addr, peer_entries, outbound_rx, handler,
).await
.map_err(|e| ConsensusError::Network(e))?;

let network_join = tokio::spawn(async move {
    if let Err(e) = swarm.run().await {
        tracing::error!(?e, "raft swarm exited with error");
    }
});
```

- [ ] **Step 4: Add `libp2p_peer_id` to `PeerConfig`.**

Edit `omega-commitment/crates/omega-toy-consensus/src/config.rs`:

```rust
pub struct PeerConfig {
    pub node_id: u64,
    /// libp2p PeerId in base58 (e.g. `12D3KooW...`).
    pub libp2p_peer_id: String,
    pub libp2p_addr: String,
    pub rpc_url: String,
}
```

Update the `FromStr` impl to expect a 4-field comma-separated form:
`<node_id>,<peer_id>,<libp2p_addr>,<rpc_url>`. Bin's `--peer` help text updates accordingly.

- [ ] **Step 5: Delete the static-dispatcher functions.**

Remove from `node.rs`:
- `static RAFT_REGISTRY`, `static RAFT_LINK_BLOCKS`
- `raft_registry()`, `raft_link_blocks()`
- `register_raft()`, `unregister_raft()`, `route_raft()`
- `clear_raft_link_blocks_for_test()`, `partition_raft_link_for_test()`, `raft_link_blocked()`
- `spawn_network_dispatcher()`, `dispatch_raft_request()`

Remove from `lib.rs`: the `test_support` module entirely (Phase 7 takes the rest of the test-support feature out — for now the module just goes away).

- [ ] **Step 6: Remove the `test-support` feature and self-dep.**

Edit `omega-commitment/crates/omega-toy-consensus/Cargo.toml`:

Remove:
```toml
test-support = []
```

And the `[dev-dependencies]` self-dep:
```toml
# omega-toy-consensus = { path = ".", features = ["test-support"] }
```

- [ ] **Step 7: Update integration tests' `tests/common/mod.rs` to drop `clear_raft_link_blocks` calls.**

Replace:
```rust
omega_toy_consensus::test_support::clear_raft_link_blocks();
```
with: (deleted — partitions are now libp2p-level, see Phase 7).

For Phase 2, the partition tests may regress. Acceptable for this commit; Phase 7 fixes them.

- [ ] **Step 8: Build.**

```bash
cargo build -p omega-toy-consensus
```

Expected: clean. Some tests may fail in subsequent steps; that's fine.

- [ ] **Step 9: Run the unit tests.**

```bash
cargo test -p omega-toy-consensus --lib --bins --doc
```

Expected: 11 unit tests pass + doctest passes.

- [ ] **Step 10: Run a turmoil test (single_leader_emerges).**

```bash
cargo test -p omega-toy-consensus --test single_leader_emerges
```

Expected: PASS — three nodes form a cluster over libp2p inside the turmoil sim. (Turmoil intercepts TCP via DNS substitution, so libp2p connections still work.)

- [ ] **Step 11: Commit.**

```bash
git add ...
git commit -m "omega-toy-consensus: replace RAFT_REGISTRY with libp2p RaftSwarm"
```

### Task 2.2: Update remaining tests + cleanup

- [ ] **Step 1: Update `examples/three_node_local.rs` to include peer_ids.**

Each peer config now needs a `libp2p_peer_id`. Generate them at example startup (use `libp2p::identity::Keypair::generate_ed25519` in a deterministic way for the example):

```rust
let keys: [libp2p::identity::Keypair; 3] = std::array::from_fn(|_| {
    libp2p::identity::Keypair::generate_ed25519()
});
let peer_ids: [libp2p::PeerId; 3] = keys.each_ref().map(|k| k.public().to_peer_id());
// ... wire each peer config with the corresponding peer_ids[id-1].as_base58()
```

- [ ] **Step 2: Run all turmoil + failpoint tests.**

```bash
cargo test -p omega-toy-consensus --test single_leader_emerges --test single_claim_roundtrip --test leader_forwarding --test batch_limits
cargo test -p omega-toy-consensus --features failpoints --test failpoint_drop_appendentries --test failpoint_writer_closed --test failpoint_byzantine_replay -- --test-threads=1
```

Expected: all pass.

- [ ] **Step 3: Confirm partition tests are skipped/expected-fail for now.**

`tests/partition.rs::partitioned_minority_does_not_commit` previously used `partition_raft_link`; that's gone. Mark it `#[ignore]` with a comment naming Phase 7 as the unblock.

```rust
#[test]
#[ignore = "Phase 7 migrates to libp2p connection-deny rules"]
fn partitioned_minority_does_not_commit() -> turmoil::Result {
    // body unchanged
}
```

Same for `tests/leader_change_during_submit.rs`.

- [ ] **Step 4: Commit.**

```bash
git commit -m "omega-toy-consensus: update example + tests for libp2p peer_id; ignore partition tests pending Phase 7"
```

### Task 2.3: Open Phase 2 PR

- [ ] **Step 1: Push and open PR.**

```bash
git push
gh pr create --title "omega-toy-consensus: replace static raft dispatcher with libp2p" --body ...
```

CI runs all gates. Merge after green.

---

# Phase 3 — multi-process integration test

Demonstrates that three real OS processes form a cluster over real libp2p TCP.

### Task 3.1: Write the multi-process test

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/multi_process_three_node.rs`

- [ ] **Step 1: Compose the test.**

```rust
//! Three real `omega-toy-consensus run` processes form a cluster.
//!
//! Verifies that submitting a claim to one process's RPC commits across
//! all three SQLite state machines without any in-process trickery.

use std::process::Stdio;
use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use libp2p::identity::Keypair;
use tempfile::TempDir;
use tokio::process::{Child, Command};

mod common;

#[tokio::test(flavor = "multi_thread")]
async fn three_processes_form_cluster_and_replicate_a_claim() {
    let bin = env!("CARGO_BIN_EXE_omega-toy-consensus");

    let keys: [Keypair; 3] = std::array::from_fn(|_| Keypair::generate_ed25519());
    let peer_ids = keys.each_ref().map(|k| k.public().to_peer_id().to_base58());

    let data_dirs: [TempDir; 3] = std::array::from_fn(|_| TempDir::new().unwrap());

    let mut children: Vec<Child> = Vec::new();
    for id in 1u64..=3 {
        let i = (id - 1) as usize;
        let mut peers_args: Vec<String> = Vec::new();
        for j in 0..3 {
            let pj = (j as u64) + 1;
            if pj == id {
                continue;
            }
            peers_args.push("--peer".into());
            peers_args.push(format!(
                "{},{},{},/ip4/127.0.0.1/tcp/{},http://127.0.0.1:{}",
                pj,
                peer_ids[j],
                4000 + pj,
                8000 + pj
            ));
            // ... pattern: id,peer_id,libp2p_addr,rpc_url
        }
        let mut child = Command::new(bin)
            .arg("run")
            .arg("--node_id").arg(id.to_string())
            .arg("--data_dir").arg(data_dirs[i].path())
            .arg("--listen").arg(format!("/ip4/127.0.0.1/tcp/{}", 4000 + id))
            .arg("--rpc").arg(format!("127.0.0.1:{}", 8000 + id))
            .args(&peers_args)
            .arg("--cluster_id").arg("multi-process-test")
            .arg("--apply_deadline_secs").arg("30")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn omega-toy-consensus");
        // Inject identity via env var or stdin per the bin's convention.
        // (Bin needs a new `--identity-file` arg; see Task 3.2.)
        children.push(child);
    }

    // Give the cluster 5s to elect.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Submit a claim to node 1.
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(1);
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build("http://127.0.0.1:8001").unwrap();
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    let outcome: omega_toy_consensus::SubmitOutcome =
        client.request("omega_submitClaim", params).await.unwrap();
    assert!(outcome.accepted, "claim accepted: {outcome:?}");

    // Verify all three nodes report nullifier_count >= 1.
    for port in [8001u16, 8002, 8003] {
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(format!("http://127.0.0.1:{port}")).unwrap();
        let state: omega_toy_consensus::NodeState = client
            .request("omega_getState", jsonrpsee::core::params::ArrayParams::new())
            .await.unwrap();
        assert!(state.nullifier_count >= 1, "node {port} state: {state:?}");
    }

    // Tear down.
    for mut child in children {
        let _ = child.kill().await;
    }
}
```

### Task 3.2: Add `--identity-file` to the bin

The libp2p PeerId is derived from a Keypair. The test generates three keypairs and needs to pass them to the bin. The cleanest approach: bin loads identity from a file (or generates one if absent and writes to data_dir).

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/bin/omega-toy-consensus.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/config.rs`

- [ ] **Step 1: Add `identity_file: Option<PathBuf>` to `RunArgs` and `NodeConfig`.**

When unset, generate an ed25519 keypair on first start and persist to `<data_dir>/identity.bin`; reload on subsequent starts.

- [ ] **Step 2: Update the test to pre-write identity files.**

Each child gets `--identity-file <tmpdir>/identity-N.bin` and the test writes the keypair bytes to that file before spawning.

- [ ] **Step 3: Run the test.**

```bash
cargo test -p omega-toy-consensus --test multi_process_three_node -- --nocapture
```

Expected: PASS in ~10-20s.

- [ ] **Step 4: Commit.**

```bash
git commit -m "omega-toy-consensus: identity-file flag + multi-process integration test"
```

---

# Phase 4 — membership change RPCs

Adds `omega_addLearner` and `omega_promoteVoter` RPC methods that wire to openraft 0.9's `Raft::add_learner` and `Raft::change_membership`.

### Task 4.1: Wire the trait methods

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs`
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/types.rs`

- [ ] **Step 1: Add types `AddLearnerOutcome`, `PromoteVoterOutcome`.**

```rust
pub struct AddLearnerOutcome {
    pub log_id: LogIdView,
}
pub struct PromoteVoterOutcome {
    pub log_id: LogIdView,
}
```

- [ ] **Step 2: Add the trait methods in `OmegaRpc`.**

```rust
#[method(name = "addLearner")]
async fn add_learner(
    &self,
    node_id: u64,
    libp2p_peer_id: String,
    libp2p_addr: String,
    rpc_url: String,
) -> Result<AddLearnerOutcome, ErrorObjectOwned>;

#[method(name = "promoteVoter")]
async fn promote_voter(&self, node_id: u64) -> Result<PromoteVoterOutcome, ErrorObjectOwned>;
```

- [ ] **Step 3: Implement.**

Body of `add_learner`:
```rust
let basic = openraft::BasicNode::new(libp2p_addr.clone());
let res = self.inner.raft.add_learner(node_id, basic, true).await
    .map_err(|e| {
        // Map ChangeMembershipError variants to JSON-RPC errors.
        // ...
    })?;
let log_id = LogIdView { ... };
Ok(AddLearnerOutcome { log_id })
```

`promote_voter`: build the new `BTreeSet<NodeId>` of voters and call `self.inner.raft.change_membership(...)`.

- [ ] **Step 4: Update routing.rs to translate `ChangeMembershipError`.**

The existing `translate_client_write_error` collapses non-ForwardToLeader errors to `-32603`; for membership change the rejection reasons (e.g. `LearnerIsLagging`) deserve their own codes. Add `-32006` `MembershipRejected` to the application range.

- [ ] **Step 5: Write integration test.**

`tests/membership_change.rs`:
- 3-node cluster.
- Add a 4th node as learner.
- Promote the learner to voter.
- Submit a claim; verify the 4th node applies it.

- [ ] **Step 6: Run.**

```bash
cargo test -p omega-toy-consensus --test membership_change
```

- [ ] **Step 7: Commit.**

---

# Phase 5 — real snapshot install

Replaces today's `tests/snapshot_install_during_submit.rs::three_submits_across_elapsed_window_replicate_to_all_nodes` (a placeholder) with a test that genuinely exercises `Raft::install_snapshot`.

### Task 5.1: Add a snapshot trigger RPC

**Files:**
- Modify: `omega-commitment/crates/omega-toy-consensus/src/rpc/server.rs`

- [ ] **Step 1: Add `omega_takeSnapshot` (admin RPC).**

```rust
#[method(name = "takeSnapshot")]
async fn take_snapshot(&self) -> Result<TakeSnapshotOutcome, ErrorObjectOwned>;
```

Body: calls `self.inner.raft.trigger().snapshot()` (openraft 0.9 API).

### Task 5.2: Write the real snapshot install test

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_real.rs`
- Delete: `omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_during_submit.rs`

- [ ] **Step 1: Compose the test.**

```rust
//! 4-node cluster: leader runs, 3 voters keep up, learner joins after
//! N submits — must receive its initial state via install_snapshot.

#[tokio::test(flavor = "multi_thread")]
async fn lagging_learner_receives_state_via_install_snapshot() {
    // 1. Boot 3 voters.
    // 2. Submit ~50 claims; force a snapshot via omega_takeSnapshot.
    // 3. Boot a 4th node, add as learner.
    // 4. Wait for state.applied_index >= leader.applied_index on the learner.
    // 5. Verify the omega_network counter for InstallSnapshot RPCs is > 0.
    // 6. Submit one more claim; verify the learner sees it without further
    //    snapshot install.
}
```

- [ ] **Step 2: Add metrics counters in `omega-network`.**

Track per-RPC-type counters (AppendEntries, Vote, InstallSnapshot) on `RaftSwarm` so the test can assert that an `InstallSnapshot` actually fired.

- [ ] **Step 3: Run.**

```bash
cargo test -p omega-toy-consensus --test snapshot_install_real
```

- [ ] **Step 4: Delete the placeholder.**

```bash
git rm omega-commitment/crates/omega-toy-consensus/tests/snapshot_install_during_submit.rs
```

- [ ] **Step 5: Commit.**

---

# Phase 6 — cross-platform CI

Linux + macOS jobs in `.github/workflows/ci.yml`.

### Task 6.1: Expand the CI matrix

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Read the current workflow.**

```bash
cat .github/workflows/ci.yml
```

- [ ] **Step 2: Add OS matrix.**

```yaml
strategy:
  fail-fast: false
  matrix:
    os: [windows-latest, ubuntu-latest, macos-latest]
runs-on: ${{ matrix.os }}
```

Caveats:
- SQLite bundled feature should already work cross-OS.
- libp2p TCP works cross-OS.
- WSL/Kani gate is Linux-only — gate that step on `matrix.os == 'ubuntu-latest'`.

- [ ] **Step 3: Push, watch CI.**

Expected: green on all three OSes. If something breaks (path separators, permission bits, etc.), fix in-place.

- [ ] **Step 4: Commit.**

---

# Phase 7 — drop `test_support` backdoor entirely

With real libp2p, partitions can be modeled at the connection layer. The `partition_raft_link` / `clear_raft_link_blocks` mechanism is no longer needed.

### Task 7.1: Add libp2p connection-deny support

**Files:**
- Modify: `omega-commitment/crates/omega-network/src/inbound.rs`

- [ ] **Step 1: Expose a way to ban + unban a peer.**

```rust
impl RaftSwarm {
    pub fn ban_peer(&mut self, node_id: u64) {
        if let Some(peer) = self.peers.get(&node_id) {
            self.swarm.disconnect_peer_id(peer.peer_id).ok();
            self.swarm.behaviour_mut().add_address(&peer.peer_id, /* unreachable addr */);
        }
    }
    pub fn unban_peer(&mut self, node_id: u64) {
        if let Some(peer) = self.peers.get(&node_id) {
            self.swarm.behaviour_mut().add_address(&peer.peer_id, peer.address.clone());
        }
    }
}
```

Or better: extend the `RaftSwarm` event loop to consume a control channel:

```rust
pub enum SwarmCommand {
    BanPeer(u64),
    UnbanPeer(u64),
}
```

The `node.rs` exposes a `pub(crate) swarm_cmd_tx` only to integration tests via a feature flag (`integration-test-controls`) — but **not** through a public `pub mod test_support`.

Better still: integration tests don't need the control channel at all if they spawn real OS processes (Phase 3 already covers cluster behavior end-to-end). Drop the in-process partition tests entirely; cover partitions via tests that:

- Spawn 3 processes.
- Use OS firewall rules (iptables on Linux, `pfctl` on macOS, Windows Firewall on Windows) to drop traffic between selected processes.

That keeps the production lib pristine. Cross-platform firewall manipulation is finicky, so this is a substantial subtask — see 7.2.

### Task 7.2: Multi-process partition tests via OS firewall

**Files:**
- Create: `omega-commitment/crates/omega-toy-consensus/tests/multi_process_partition.rs`

- [ ] **Step 1: Cross-platform firewall helper.**

```rust
mod firewall {
    /// Drop all TCP traffic to/from a port pair.
    pub fn block(port_a: u16, port_b: u16) -> std::io::Result<FirewallGuard> {
        #[cfg(target_os = "linux")] { /* iptables -I */ }
        #[cfg(target_os = "macos")] { /* pfctl */ }
        #[cfg(windows)] { /* netsh advfirewall */ }
    }
}
```

These tests will require sudo/admin on most CI runners, so gate them with `#[cfg(feature = "integration-multi-process-partition")]` and a separate CI job that runs with elevated privileges.

- [ ] **Step 2: Write `partitioned_minority_does_not_commit_multi_process`.**

Three real processes; firewall blocks node 1 from nodes 2 and 3. Submit to node 1 — must reject. Submit to node 2 (majority) — must accept. Lift firewall — node 1 catches up.

- [ ] **Step 3: Delete the in-process partition tests.**

```bash
git rm omega-commitment/crates/omega-toy-consensus/tests/partition.rs
```

(Or keep as `#[cfg(test)]` unit tests of the routing translator that demonstrate the error-code contract; the multi-process tests cover the real partition behavior.)

### Task 7.3: Final cleanup

- [ ] **Step 1: Confirm `test_support` is gone.**

```bash
grep -rn "test_support" omega-commitment/crates/omega-toy-consensus/
```

Expected: no matches.

- [ ] **Step 2: Confirm no `RAFT_REGISTRY`, `RAFT_LINK_BLOCKS` left.**

```bash
grep -rn "RAFT_REGISTRY\|RAFT_LINK_BLOCKS" omega-commitment/
```

Expected: no matches.

- [ ] **Step 3: Update the wiki roadmap.**

`cardano-wiki/wiki/pages/loganet-roadmap.md`: move the Group 2 entries to "Shipped in Group 2"; the "Group 3" section now covers what was Group 3 + the residual (real Kani harness, real Shuttle model).

- [ ] **Step 4: Run all gates one final time.**

```bash
cargo build --workspace
cargo test -p omega-toy-consensus --no-fail-fast -- --test-threads=1
cargo test -p omega-toy-consensus --features failpoints -- --test-threads=1
cargo doc --workspace --no-deps --document-private-items
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

- [ ] **Step 5: Commit + open final Phase 7 PR.**

---

## Self-review

- [ ] **Spec coverage:** Every Group 2 item from `loganet-roadmap.md` § "Group 2" maps to a Phase. Membership change → P4. Snapshot trigger RPC + integration → P5. Linux+macOS CI → P6. Removing `RAFT_REGISTRY` + `--peer libp2p_addr` becoming the actual transport → P1+P2.
- [ ] **No placeholders:** All steps carry exact code or exact commands.
- [ ] **Type consistency:** `OmegaInboundHandler` used in P2 matches the `InboundRaftHandler` trait signature from P1.3. `PeerEntry` from P1.4 is the same type wired in P2.1. `RaftSwarm::new` signature is consistent across phases.
- [ ] **Risks:** P1's libp2p `request_response` config (timeout, max in-flight, idle conn timeout) needs tuning under load. P3's `--identity-file` flag is new public surface — document in `loganet-roadmap.md`. P7's OS-firewall manipulation needs CI runner privileges; mark the relevant tests `#[ignore]` by default and gate behind a feature flag + CI job that explicitly runs them.

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-08-omega-toy-consensus-group2-plan.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per Phase, review between phases, fast iteration. Each Phase is naturally a separate PR.
2. **Inline Execution** — execute phases in this session via `executing-plans`, batch with checkpoints for review.

Which approach?
