# Skills Tooling Audit

_Source: parallel agent pass, 2026-05-03. Files audited: `skills/{README.md, manifest.toml, install.sh}`, `skills/local/plonky3-friendly-rust/SKILL.md`._

## Manifest contents

| Source type | Count | Names |
|---|---|---|
| `[[skill.local]]` | 1 | `plonky3-friendly-rust` |
| `[[skill.git_root]]` | 2 | `humanizer`, `rust-skills` |
| `[[skill.git_subpath]]` | 6 | `ascii-visualizer`, `rust-engineer`, `code-reviewer`, `test-master`, `security-reviewer`, `debugging-wizard` |
| `[[skill.git_bulk]]` | 1 group | `ab-*` (38 skills) from `actionbook/rust-skills` |
| `[[plugin]]` | 2 | `superpowers` (obra/superpowers-marketplace), `wiki-skills-v2` (kfchou/wiki-skills) — manual `/plugin` activation |
| `[[cargo_install]]` | 4 | `cargo-fuzz`, `cargo-mutants`, `kani-verifier` (post: `cargo kani setup`), `cargo-nextest` |
| `[[rustup_component]]` | 2 toolchains | stable: clippy, rustfmt, rust-src, llvm-tools-preview · nightly: miri, rust-src |

Total skills: 1 + 2 + 6 + 38 = 47 + 2 plugin packages.

## Installer design

- **Idempotency**: existing skills skipped unless `--force` (install.sh:108, 120, 134, 166).
- **Modes**: `--dry-run`, `--force`, `--skills`, `--cargo`, `--rustup`, `--verify`, `-h/--help`.
- **TOML parser**: embedded Python 3 one-liner using `tomllib.loads()` (install.sh:68-88), Python 3.11+. Emits TSV records for bash.
- **Source handlers** (install.sh:98-173):
  - `local`: copy from `skills/<src>` to `~/.claude/skills/<name>`.
  - `git_root`: shallow clone, copy entire repo as the skill.
  - `git_subpath`: shallow clone, copy one subdirectory; warns if missing (line 143).
  - `git_bulk`: shallow clone, iterate subdirs, install each with `SKILL.md`; logs count.
- **Cargo/rustup** (install.sh:179-221): checks command presence; maps `kani-verifier` → `cargo-kani`; runs post-install hook as bash; auto-installs only `stable`/`nightly`.
- **Verify mode** (install.sh:227-253): prints checkmark/cross per skill, counts prefix matches for bulk, lists plugins requiring manual activation.

## Vendored skill: plonky3-friendly-rust

Located at `skills/local/plonky3-friendly-rust/SKILL.md` (182 lines). Covers Rust patterns that compile cleanly to plonky3 STARK circuits.

Key patterns:

- **Hash selection** (lines 23-35): "every hash that runs inside the verifier circuit should be Poseidon2 unless there is an explicit interoperability reason." Costs: Poseidon2 1x, Blake3 ~24x, Keccak ~30x. SHA-256/Blake2b not native.
- **Domain separation** (lines 42-66): "non-negotiable. Without it, an attacker can present an internal node's preimage…as a 'leaf'." Cites Uniswap, OpenZeppelin, airdrop systems.
- **Serialization** (lines 72-80): deterministic encoding only; no `HashMap` iteration order; no sub-second timestamps unless spec pins them. Reject trailing bytes.
- **Witness/public split** (lines 97-116): claim-transaction pattern with constraint checklist; skipping any constraint = soundness failure.
- **Anti-patterns** (118-130): no curve ops (10⁴-10⁵x hash cost), no `String` allocations, no `Box<dyn Trait>`, no `unwrap()`/`panic!`, no `HashMap` iteration (use `BTreeMap`), no wall-clock-dependent values.
- **Preferred** (132-144): pure functions, const-generic arrays, trait abstraction at the boundary not inside, `#![forbid(unsafe_code)]`, property-based tests, domain-typed newtypes.

This skill is the only repo-vendored one and is directly relevant to the omega-commitment T1 → T6 verifier circuit work.

## Fragility and gaps

- **Python dependency**: TOML parsing requires Python 3.11+ `tomllib`; no fallback parser.
- **Windows compatibility**: install.sh is bash-only; no `.bat`/PowerShell variant. (Confirmed during initial install attempt: CRLF line endings broke initial run on this machine; resolved via `git config core.autocrlf false` then re-clone.) `rsync` mentioned in README.md:17 not tested on Windows paths.
- **Plugin activation gap**: `/plugin marketplace add` and `/plugin install` cannot be auto-run because the harness blocks edits to `~/.claude/settings.json`. Installer prints instructions but does not verify execution. README.md:46-57 documents the gap; users may miss the two commands.
- **Upstream commits unpinned**: all git installs use `git clone --depth 1`. No way to pin upstream skill versions; if an upstream skill breaks, the next `--force` pull fetches the broken version.
- **Bulk install silent skip**: `install_skill_git_bulk` silently skips any subdir without `SKILL.md` (install.sh:160-163). Renames upstream → silent disappearance.
- **Subpath validation deferred**: `--dry-run` does not check whether subpaths actually exist; only discovered at install time (line 142).
- **Cargo binary aliasing silent**: `kani-verifier → cargo-kani` mapping at install.sh:189 emits no log output.
- **No `--uninstall`**: users must manually delete from `~/.claude/skills/`.
- **No version snapshot**: no record of which upstream commits were installed; reproducibility weak.
