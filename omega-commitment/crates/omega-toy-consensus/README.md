# omega-toy-consensus

LoganNet keystone: openraft 0.9 + omega-mock-ledger + omega-network + minimal
JSON-RPC. Library + `omega-toy-consensus run` binary.

See `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md` for the
full design and `cardano-wiki/wiki/pages/loganet-roadmap.md` for the milestone
roadmap and Group 2 deferrals.

## v0.1 limitations

- Localhost only (`127.0.0.1:800N`).
- No TLS, no auth, no rate limiting.
- Two RPC methods: `omega_submitClaim`, `omega_getState`.
- No membership change; static `--peer` topology.
- Windows + 1.95.0 toolchain only.
