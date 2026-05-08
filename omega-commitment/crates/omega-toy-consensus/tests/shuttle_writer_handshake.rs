//! Shuttle concurrency model for the writer-channel handshake.
//!
//! NOTE — STRUCTURAL PLACEHOLDER: The model below is a generic
//! `(u64, Sender<bool>)` mpsc handshake — 5 submitters and 1 writer that
//! accepts 3 messages then drops. It does NOT use
//! `omega_mock_ledger::WriterHandle` or `apply_claim_tx`; the actual
//! writer-actor request/reply protocol (oneshot replies, retry on
//! channel-close, SQLite transaction boundaries) is not exercised. This
//! test exists to wire the shuttle-loom gate, not to bind verification of
//! the actual handshake; replacing it with a real model is Group 3 work.
//! See `cardano-wiki/wiki/pages/loganet-roadmap.md` § "Toy verification
//! harnesses". Also gated behind `feature = "shuttle-tests"`, which the
//! standard `cargo test` gates do not enable — run via
//! `cargo test -p omega-toy-consensus --features shuttle-tests`.

#[cfg(feature = "shuttle-tests")]
mod tests {
    use std::time::Duration;

    use shuttle::sync::mpsc;
    use shuttle::thread;

    fn run() {
        let (tx, rx) = mpsc::channel::<(u64, std::sync::mpsc::Sender<bool>)>();

        let writer = thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((_index, reply)) = rx.recv() {
                    let _ = reply.send(true);
                }
            }
            drop(rx);
        });

        let mut submitters = Vec::new();
        for index in 0..5 {
            let tx = tx.clone();
            submitters.push(thread::spawn(move || {
                let (reply, reply_rx) = std::sync::mpsc::channel();
                if tx.send((index, reply)).is_err() {
                    return;
                }
                let _ = reply_rx.recv_timeout(Duration::from_millis(50));
            }));
        }

        for submitter in submitters {
            submitter.join().unwrap();
        }
        writer.join().unwrap();
    }

    #[test]
    fn shuttle_writer_handshake() {
        shuttle::check_random(run, 100);
    }
}
