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
- **Raft RPC is in-process.** The `--peer <id>,<libp2p_addr>,<rpc_url>`
  argument is recorded for leader-hint resolution and openraft's static
  membership table only; the `libp2p_addr` is **not** wired into raft
  RPC at v0.1. Three independent `omega-toy-consensus run` processes
  cannot form a cluster — only `examples/three_node_local`, which
  spawns three nodes inside one tokio runtime, does. See
  `cardano-wiki/wiki/pages/loganet-roadmap.md` § "Group 1 transport".
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
