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
/// default chunk is 1 MiB, and synthetic claim fixtures can produce larger
/// append-entry envelopes. Keep the transport frame cap aligned with the CBOR
/// envelope cap enforced by [`crate::rpc::decode_cbor`].
pub const MAX_FRAME_BYTES: usize = crate::rpc::MAX_RAFT_RPC_BYTES;

/// CBOR codec for `RaftRpcRequest` / `RaftRpcResponse` over libp2p.
#[derive(Clone, Default)]
pub struct RaftCodec;

#[async_trait]
impl Codec for RaftCodec {
    type Protocol = StreamProtocol;
    type Request = RaftRpcRequest;
    type Response = RaftRpcResponse;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
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
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame {} bytes exceeds max {MAX_FRAME_BYTES}", bytes.len()),
        ));
    }
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

#[cfg(test)]
mod tests {
    use futures::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn frame_round_trips_payload_above_old_four_mib_cap() {
        let payload = vec![7u8; 4 * 1024 * 1024 + 1];
        let mut io = Cursor::new(Vec::new());

        write_frame(&mut io, &payload).await.unwrap();
        io.set_position(0);
        let decoded = read_frame(&mut io).await.unwrap();

        assert_eq!(decoded.len(), payload.len());
        assert_eq!(decoded[0], 7);
    }

    #[tokio::test]
    async fn write_frame_rejects_payload_above_envelope_cap() {
        let payload = vec![0u8; MAX_FRAME_BYTES + 1];
        let mut io = Cursor::new(Vec::new());

        let err = write_frame(&mut io, &payload).await.unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
