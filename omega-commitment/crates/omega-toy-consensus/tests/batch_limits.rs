//! JSON-RPC batch limit tests.

mod common;

use std::sync::Mutex;
use std::time::Duration;

use jsonrpsee::core::client::ClientT;

static TEST_LOCK: Mutex<()> = Mutex::new(());

async fn leader_client() -> jsonrpsee::http_client::HttpClient {
    let leader_url = common::leader_url().await;
    jsonrpsee::http_client::HttpClientBuilder::default()
        .build(leader_url)
        .unwrap()
}

fn get_state_batch(len: usize) -> jsonrpsee::core::params::BatchRequestBuilder<'static> {
    let mut batch = jsonrpsee::core::params::BatchRequestBuilder::new();
    for _ in 0..len {
        batch
            .insert("omega_getState", jsonrpsee::rpc_params![])
            .unwrap();
    }
    batch
}

fn raw_get_state_batch(len: usize) -> String {
    let requests: Vec<_> = (0..len)
        .map(|id| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "omega_getState",
                "params": [],
            })
        })
        .collect();
    serde_json::to_string(&requests).unwrap()
}

fn response_complete(response: &[u8]) -> bool {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let content_length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    match content_length {
        Some(len) => response.len() >= header_end + 4 + len,
        None => true,
    }
}

async fn raw_post(url: &str, body: String) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let port = url.rsplit(':').next().unwrap().parse::<u16>().unwrap();
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let request = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer)).await;
        match read {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                response.extend_from_slice(&buffer[..n]);
                if response_complete(&response) {
                    break;
                }
            }
            Ok(Err(error)) => panic!("raw batch read failed: {error}"),
            Err(_elapsed) if !response.is_empty() => break,
            Err(_elapsed) => panic!("raw batch read timed out"),
        }
    }
    String::from_utf8(response).unwrap()
}

#[test]
fn batch_at_cap_succeeds() -> turmoil::Result {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        let client = leader_client().await;
        let response = client
            .batch_request::<omega_toy_consensus::NodeState>(get_state_batch(25))
            .await
            .unwrap();
        assert_eq!(response.len(), 25);
        assert_eq!(response.num_successful_calls(), 25);
        Ok(())
    });

    sim.run()
}

#[test]
fn batch_over_cap_rejected() -> turmoil::Result {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        let leader_url = common::leader_url().await;
        let body = raw_get_state_batch(26);
        let response = raw_post(&leader_url, body).await;
        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["error"]["code"], -32600);
        Ok(())
    });

    sim.run()
}
