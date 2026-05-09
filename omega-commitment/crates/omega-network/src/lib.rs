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
//! network ordering and framing: Raft RPC payloads are CBOR encoded with
//! envelope-byte and recursion-depth caps before deserialisation, and the
//! [`snapshot`] module defines a serial, one-chunk-at-a-time wire protocol
//! whose receiver hashes-then-installs so unordered libp2p responses cannot
//! reorder or forge the installed bytes.
//!
//! # v0.1 limitations
//!
//! - The openraft `RaftNetwork::install_snapshot` adapter at
//!   [`network::LibP2pNetwork`] currently ships the entire
//!   `InstallSnapshotRequest` as a single CBOR payload bounded by
//!   [`rpc::MAX_RAFT_RPC_BYTES`]. The serial chunking + fsync-before-ack
//!   receiver in [`snapshot::SnapshotFileReceiver`] is **not yet wired into
//!   that path**; it is the wire protocol the node-runner crate will drive
//!   directly when snapshots outgrow the single-payload bound. Until that
//!   wiring lands, snapshots travel through the request-response actor as
//!   one CBOR envelope, gated only by `MAX_RAFT_RPC_BYTES`.
//! - The inbound dispatcher is out of scope for this crate; soundness
//!   against a byzantine peer additionally requires the inbound layer to
//!   bind the libp2p `PeerId` of each connection to the openraft `NodeId`
//!   declared in the envelope. See the `network` module's source for the
//!   full contract.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod discovery;
pub mod inbound;
pub mod protocol;
pub mod rpc;
pub mod snapshot;

mod network;

pub use inbound::InboundRaftHandler;
pub use network::{
    LibP2pNetwork, LibP2pNetworkFactory, OutboundRaftRequest, DEFAULT_OUTBOUND_CAPACITY,
};
pub use protocol::{RaftCodec, MAX_FRAME_BYTES, RAFT_PROTOCOL};
