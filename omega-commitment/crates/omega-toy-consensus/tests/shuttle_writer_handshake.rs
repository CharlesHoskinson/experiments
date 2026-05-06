//! Shuttle concurrency model for the writer-channel handshake.

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
