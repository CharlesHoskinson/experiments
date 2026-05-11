# omega-toy-consensus

LoganNet keystone: openraft 0.9 + omega-mock-ledger + omega-network + minimal
JSON-RPC. Library + `omega-toy-consensus run` binary.

See `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md` for the
full design and `cardano-wiki/wiki/pages/loganet-roadmap.md` for the
milestone roadmap and Group 2 deferrals.

## v0.1 limitations

- Localhost only (`127.0.0.1:800N`); loopback bind enforced.
- No TLS, no auth, no rate limiting.
- Two RPC methods: `omega_submitClaim`, `omega_getState`.
- No membership change; static `--peer` topology.
- Raft RPC uses static libp2p request-response peers. The `--peer`
  argument is `<id>,<peer_id>,<libp2p_addr>,<rpc_url>`, and each node uses a
  persisted libp2p identity keypair. Discovery remains out of scope.
- Windows + 1.95.0 toolchain only.

## Test pack honesty

- The Kani harness in `kani-proofs/` is a **toy state-machine
  placeholder**, not binding verification of `MockLedger::restore_snapshot`.
- The Shuttle test in `tests/shuttle_writer_handshake.rs` is a
  **generic mpsc handshake model**, not the actual writer-actor
  request/reply protocol.

Both are kept in tree to wire the gates and to capture scaffolding
for Group 3's real harnesses. See
`cardano-wiki/wiki/pages/loganet-roadmap.md` § "Toy verification
harnesses".
