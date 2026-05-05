//! JSON-RPC server.

pub mod error;
pub mod server;
pub mod types;

pub use server::{OmegaRpcImpl, OmegaRpcServer};
