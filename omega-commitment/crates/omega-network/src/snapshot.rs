//! Explicit snapshot chunking protocol for request-response transport.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use thiserror::Error;

/// Maximum snapshot chunk payload accepted by the v0.1 wire protocol.
pub const MAX_SNAPSHOT_CHUNK_BYTES: usize = 1024 * 1024;

/// Snapshot transfer initialization frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotInit {
    /// Opaque openraft snapshot id.
    pub snapshot_id: String,
    /// Total number of data chunks expected before finalization.
    pub total_chunks: u64,
    /// Total unchunked snapshot byte length.
    pub total_bytes: u64,
    /// SHA3-256 digest of the full unchunked snapshot.
    pub sha3_of_full: [u8; 32],
}

/// Snapshot transfer data chunk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotChunk {
    /// Opaque openraft snapshot id.
    pub snapshot_id: String,
    /// Zero-based chunk index.
    pub chunk_idx: u64,
    /// Raw snapshot bytes for this chunk.
    pub payload: Vec<u8>,
}

/// Snapshot transfer finalization frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotFinalize {
    /// Opaque openraft snapshot id.
    pub snapshot_id: String,
    /// SHA3-256 digest of the full unchunked snapshot.
    pub sha3_of_full: [u8; 32],
}

/// Snapshot request-response wire frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotFrame {
    /// Starts a new snapshot transfer.
    Init(SnapshotInit),
    /// Carries one ordered snapshot data chunk.
    Chunk(SnapshotChunk),
    /// Finalizes the staged snapshot.
    Finalize(SnapshotFinalize),
}

/// Acknowledgement returned after a receiver durably handles a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotAck {
    /// The frame was accepted and the receiver expects `next_chunk_idx`.
    Accepted {
        /// Opaque openraft snapshot id.
        snapshot_id: String,
        /// Next chunk index expected by the receiver.
        next_chunk_idx: u64,
    },
    /// The snapshot was verified and installed.
    Complete {
        /// Opaque openraft snapshot id.
        snapshot_id: String,
        /// Final installed snapshot path.
        installed_path: PathBuf,
    },
}

/// Protocol-level snapshot transfer errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotProtocolError {
    /// A snapshot is already active.
    #[error("snapshot session already active")]
    ActiveSnapshot,
    /// A chunk or finalize frame arrived before an init frame.
    #[error("no active snapshot session")]
    NoActiveSnapshot,
    /// The frame snapshot id did not match the active session.
    #[error("snapshot id mismatch: expected {expected}, got {actual}")]
    SnapshotIdMismatch {
        /// Active snapshot id.
        expected: String,
        /// Incoming frame snapshot id.
        actual: String,
    },
    /// A chunk arrived out of order.
    #[error("out-of-order snapshot chunk: expected {expected}, got {actual}")]
    OutOfOrderChunk {
        /// Expected chunk index.
        expected: u64,
        /// Incoming chunk index.
        actual: u64,
    },
    /// A chunk arrived after the declared total chunk count.
    #[error("unexpected snapshot chunk after declared total: total {total_chunks}, got {actual}")]
    UnexpectedChunk {
        /// Declared total chunk count.
        total_chunks: u64,
        /// Incoming chunk index.
        actual: u64,
    },
    /// A chunk exceeded the maximum v0.1 chunk size.
    #[error("snapshot chunk too large: {actual} > {max}")]
    ChunkTooLarge {
        /// Incoming chunk length.
        actual: usize,
        /// Maximum allowed chunk length.
        max: usize,
    },
    /// The accepted bytes would exceed the declared total.
    #[error("snapshot bytes exceed declared total: {actual} > {expected}")]
    BytesExceedTotal {
        /// Incoming cumulative byte length.
        actual: u64,
        /// Declared total byte length.
        expected: u64,
    },
    /// Finalization arrived before every declared chunk was accepted.
    #[error("snapshot finalized before all chunks: expected {expected}, got {actual}")]
    FinalizeBeforeAllChunks {
        /// Declared total chunk count.
        expected: u64,
        /// Chunks accepted so far.
        actual: u64,
    },
    /// The final byte count did not match the declared byte count.
    #[error("snapshot byte count mismatch: expected {expected}, got {actual}")]
    ByteCountMismatch {
        /// Declared total byte length.
        expected: u64,
        /// Accepted byte length.
        actual: u64,
    },
    /// The final SHA3 digest did not match the staged snapshot bytes.
    #[error("snapshot sha3 digest mismatch")]
    HashMismatch,
}

/// Errors returned by the disk-backed snapshot receiver.
#[derive(Debug, Error)]
pub enum SnapshotReceiveError {
    /// The frame violated the snapshot wire protocol.
    #[error(transparent)]
    Protocol(#[from] SnapshotProtocolError),
    /// Filesystem I/O failed while staging or installing a snapshot.
    #[error("snapshot file io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Builds serial request-response frames for a full snapshot byte buffer.
pub fn chunk_snapshot_bytes(snapshot_id: impl Into<String>, bytes: &[u8]) -> Vec<SnapshotFrame> {
    let snapshot_id = snapshot_id.into();
    let digest = sha3_256(bytes);
    let total_chunks = if bytes.is_empty() {
        0
    } else {
        bytes.len().div_ceil(MAX_SNAPSHOT_CHUNK_BYTES) as u64
    };

    let mut frames = Vec::with_capacity(total_chunks as usize + 2);
    frames.push(SnapshotFrame::Init(SnapshotInit {
        snapshot_id: snapshot_id.clone(),
        total_chunks,
        total_bytes: bytes.len() as u64,
        sha3_of_full: digest,
    }));
    for (chunk_idx, payload) in bytes.chunks(MAX_SNAPSHOT_CHUNK_BYTES).enumerate() {
        frames.push(SnapshotFrame::Chunk(SnapshotChunk {
            snapshot_id: snapshot_id.clone(),
            chunk_idx: chunk_idx as u64,
            payload: payload.to_vec(),
        }));
    }
    frames.push(SnapshotFrame::Finalize(SnapshotFinalize {
        snapshot_id,
        sha3_of_full: digest,
    }));
    frames
}

/// Disk-backed snapshot receiver that fsyncs before acknowledging chunks.
pub struct SnapshotFileReceiver {
    staging_dir: PathBuf,
    installed_path: PathBuf,
    active: Option<ActiveSnapshot>,
}

impl SnapshotFileReceiver {
    /// Creates a snapshot receiver.
    pub fn new(staging_dir: impl AsRef<Path>, installed_path: impl AsRef<Path>) -> Self {
        Self {
            staging_dir: staging_dir.as_ref().to_path_buf(),
            installed_path: installed_path.as_ref().to_path_buf(),
            active: None,
        }
    }

    /// Returns the active staged path, if a transfer is active.
    pub fn active_staged_path(&self) -> Option<&Path> {
        self.active
            .as_ref()
            .map(|active| active.staged_path.as_path())
    }

    /// Receives, validates, and durably acknowledges one snapshot frame.
    pub fn receive(&mut self, frame: SnapshotFrame) -> Result<SnapshotAck, SnapshotReceiveError> {
        match frame {
            SnapshotFrame::Init(init) => self.receive_init(init),
            SnapshotFrame::Chunk(chunk) => self.receive_chunk(chunk),
            SnapshotFrame::Finalize(finalize) => self.receive_finalize(finalize),
        }
    }

    /// Aborts the active transfer after a leader-change event.
    pub fn abort_on_leader_change(&mut self) -> Result<bool, SnapshotReceiveError> {
        let Some(active) = self.active.take() else {
            return Ok(false);
        };
        drop(active.file);
        remove_if_exists(&active.staged_path)?;
        Ok(true)
    }

    fn receive_init(&mut self, init: SnapshotInit) -> Result<SnapshotAck, SnapshotReceiveError> {
        if self.active.is_some() {
            return Err(SnapshotProtocolError::ActiveSnapshot.into());
        }
        std::fs::create_dir_all(&self.staging_dir)?;
        let staged_path = self.staged_path(&init.snapshot_id);
        remove_if_exists(&staged_path)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)?;
        file.sync_all()?;
        self.active = Some(ActiveSnapshot {
            init,
            next_chunk_idx: 0,
            bytes_received: 0,
            staged_path,
            file,
        });
        let snapshot_id = self
            .active
            .as_ref()
            .map(|active| active.init.snapshot_id.clone())
            .ok_or(SnapshotProtocolError::NoActiveSnapshot)?;
        Ok(SnapshotAck::Accepted {
            snapshot_id,
            next_chunk_idx: 0,
        })
    }

    fn receive_chunk(&mut self, chunk: SnapshotChunk) -> Result<SnapshotAck, SnapshotReceiveError> {
        let active = self
            .active
            .as_mut()
            .ok_or(SnapshotProtocolError::NoActiveSnapshot)?;
        if chunk.snapshot_id != active.init.snapshot_id {
            return Err(SnapshotProtocolError::SnapshotIdMismatch {
                expected: active.init.snapshot_id.clone(),
                actual: chunk.snapshot_id,
            }
            .into());
        }
        if chunk.chunk_idx != active.next_chunk_idx {
            return Err(SnapshotProtocolError::OutOfOrderChunk {
                expected: active.next_chunk_idx,
                actual: chunk.chunk_idx,
            }
            .into());
        }
        if active.next_chunk_idx >= active.init.total_chunks {
            return Err(SnapshotProtocolError::UnexpectedChunk {
                total_chunks: active.init.total_chunks,
                actual: chunk.chunk_idx,
            }
            .into());
        }
        if chunk.payload.len() > MAX_SNAPSHOT_CHUNK_BYTES {
            return Err(SnapshotProtocolError::ChunkTooLarge {
                actual: chunk.payload.len(),
                max: MAX_SNAPSHOT_CHUNK_BYTES,
            }
            .into());
        }
        let new_total = active.bytes_received + chunk.payload.len() as u64;
        if new_total > active.init.total_bytes {
            return Err(SnapshotProtocolError::BytesExceedTotal {
                actual: new_total,
                expected: active.init.total_bytes,
            }
            .into());
        }

        active.file.write_all(&chunk.payload)?;
        active.file.sync_all()?;
        active.bytes_received = new_total;
        active.next_chunk_idx += 1;
        Ok(SnapshotAck::Accepted {
            snapshot_id: active.init.snapshot_id.clone(),
            next_chunk_idx: active.next_chunk_idx,
        })
    }

    fn receive_finalize(
        &mut self,
        finalize: SnapshotFinalize,
    ) -> Result<SnapshotAck, SnapshotReceiveError> {
        let active = self
            .active
            .take()
            .ok_or(SnapshotProtocolError::NoActiveSnapshot)?;
        if finalize.snapshot_id != active.init.snapshot_id {
            let expected = active.init.snapshot_id.clone();
            let actual = finalize.snapshot_id;
            remove_active(active)?;
            return Err(SnapshotProtocolError::SnapshotIdMismatch { expected, actual }.into());
        }
        if active.next_chunk_idx != active.init.total_chunks {
            let error = SnapshotProtocolError::FinalizeBeforeAllChunks {
                expected: active.init.total_chunks,
                actual: active.next_chunk_idx,
            };
            remove_active(active)?;
            return Err(error.into());
        }
        if active.bytes_received != active.init.total_bytes {
            let error = SnapshotProtocolError::ByteCountMismatch {
                expected: active.init.total_bytes,
                actual: active.bytes_received,
            };
            remove_active(active)?;
            return Err(error.into());
        }
        if finalize.sha3_of_full != active.init.sha3_of_full {
            remove_active(active)?;
            return Err(SnapshotProtocolError::HashMismatch.into());
        }

        active.file.sync_all()?;
        let staged_path = active.staged_path.clone();
        let snapshot_id = active.init.snapshot_id.clone();
        drop(active.file);
        let staged_digest = sha3_256_file(&staged_path)?;
        if staged_digest != finalize.sha3_of_full {
            remove_if_exists(&staged_path)?;
            return Err(SnapshotProtocolError::HashMismatch.into());
        }
        remove_if_exists(&self.installed_path)?;
        std::fs::rename(&staged_path, &self.installed_path)?;
        Ok(SnapshotAck::Complete {
            snapshot_id,
            installed_path: self.installed_path.clone(),
        })
    }

    fn staged_path(&self, snapshot_id: &str) -> PathBuf {
        let digest = sha3_256(snapshot_id.as_bytes());
        let name = format!("snapshot-{}.part", hex::encode(&digest[..8]));
        self.staging_dir.join(name)
    }
}

struct ActiveSnapshot {
    init: SnapshotInit,
    next_chunk_idx: u64,
    bytes_received: u64,
    staged_path: PathBuf,
    file: File,
}

fn sha3_256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn sha3_256_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha3_256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    Ok(out)
}

fn remove_active(active: ActiveSnapshot) -> Result<(), std::io::Error> {
    let path = active.staged_path.clone();
    drop(active.file);
    remove_if_exists(&path)
}

fn remove_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
