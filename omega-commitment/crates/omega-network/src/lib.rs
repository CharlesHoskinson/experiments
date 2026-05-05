//! Libp2p transport primitives for the Omega proof experiment harness.
//!
//! # Overview
//!
//! `omega-network` is the openraft-facing transport shim used by the local
//! proof harness. It defines the CBOR request-response envelope for Raft RPCs,
//! the explicit snapshot chunking protocol, and discovery configuration shared
//! by the later node runner.
//!
//! # Design context
//!
//! - OpenSpec change: [`add-proof-experiment-harness`][1].
//! - Network design: [`omega-network`][2].
//!
//! [1]: ../../../openspec/changes/add-proof-experiment-harness/
//! [2]: ../../../openspec/changes/add-proof-experiment-harness/design.md#omega-network--libp2p-transport
//!
//! # Tier of trust
//!
//! This crate does not verify claim proofs itself. Its trust boundary is
//! network ordering and framing: Raft RPC payloads are CBOR encoded before
//! entering the libp2p request-response actor, and snapshots move through a
//! serial, one-chunk-at-a-time protocol so unordered libp2p responses cannot
//! reorder the installed bytes.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod discovery;
pub mod rpc;
pub mod snapshot;

mod network;

pub use network::{LibP2pNetwork, LibP2pNetworkFactory, OutboundRaftRequest};
