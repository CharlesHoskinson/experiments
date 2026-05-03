# Skill bundle for the experiments repo

Reproduce the full Claude Code skill + cargo verification stack on a fresh machine. After cloning this repo:

```bash
./skills/install.sh
```

That is the whole story. The script reads [`manifest.toml`](./manifest.toml) and installs every skill, cargo binary, and rustup component the project depends on. Re-running it is safe (idempotent); pass `--force` to overwrite existing skills.

## What it installs

**Custom skill (vendored in this repo):**

| Skill | Source | Purpose |
|---|---|---|
| `plonky3-friendly-rust` | [`local/plonky3-friendly-rust/`](./local/plonky3-friendly-rust/) | Patterns for Rust that compiles cleanly to plonky3 STARK constraints (Merkle trees, hash-only ops, domain separation, witness/public-input split) |

**Third-party skills (cloned from upstream into `~/.claude/skills/<name>/`):**

| Skill | Source | Purpose |
|---|---|---|
| `humanizer` | [blader/humanizer](https://github.com/blader/humanizer) | Strip AI-tells from prose |
| `ascii-visualizer` | [ArieGoldkin/devPrepAi](https://github.com/ArieGoldkin/devPrepAi/tree/main/.claude/skills/ascii-visualizer) | Box-drawing ASCII diagrams |
| `rust-skills` | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) | 179 idiomatic Rust rules |
| `rust-engineer`, `code-reviewer`, `test-master`, `security-reviewer`, `debugging-wizard`, `architecture-designer` | [Jeffallan/claude-skills](https://github.com/Jeffallan/claude-skills) | Specialist roles for Rust authoring, PR review, test design, security, debugging, ADRs |
| `ab-*` (38 skills, prefix `ab-`) | [actionbook/rust-skills](https://github.com/actionbook/rust-skills) | Rust ownership, concurrency, type-driven design, anti-patterns, LSP-driven analysis, daily-news lookup |

**Cargo verification stack** (installed via `cargo install`):

| Tool | Purpose |
|---|---|
| `cargo-fuzz` | Coverage-guided fuzzing of CBOR parsers |
| `cargo-mutants` | Mutation testing for the golden-vector layers |
| `cargo-kani` (+ CBMC backend via `cargo kani setup`) | Model-checking primitives |
| `cargo-nextest` | Faster test runner |

**Rustup components:**

| Toolchain | Components |
|---|---|
| stable | clippy, rustfmt, rust-src, llvm-tools-preview |
| nightly | miri, rust-src |

## What needs manual activation

Two Claude Code plugins are part of the workflow but cannot be auto-installed because the harness blocks programmatic edits to `~/.claude/settings.json` (counts as agent self-modification). After running `install.sh`, run these inside Claude Code:

```
/plugin marketplace add obra/superpowers-marketplace
/plugin install superpowers@superpowers-marketplace

/plugin marketplace add kfchou/wiki-skills
/plugin install wiki-skills-v2@wiki-skills
```

The plugin selection persists in `~/.claude/settings.json` after the first activation.

## Layout

```
skills/
├── README.md            this file
├── manifest.toml        declarative source list (skills, plugins, cargo, rustup)
├── install.sh           idempotent installer; reads manifest.toml
└── local/               skills authored in this project (vendored)
    └── plonky3-friendly-rust/
        └── SKILL.md
```

## Common operations

| Want to... | Run |
|---|---|
| Reproduce on a fresh machine | `./skills/install.sh` |
| Preview without doing anything | `./skills/install.sh --dry-run` |
| Re-pull updated upstream skills | `./skills/install.sh --force` |
| Check what is installed | `./skills/install.sh --verify` |
| Install only the skills (skip cargo/rustup) | `./skills/install.sh --skills` |
| Install only the cargo binaries | `./skills/install.sh --cargo` |
| Install only the rustup components | `./skills/install.sh --rustup` |
| Add a new skill to the bundle | edit `manifest.toml`, re-run installer |

## Adding a new skill

To add a third-party skill from a git repo:

```toml
# In manifest.toml, add one of:

# Single-file SKILL.md at repo root
[[skill.git_root]]
name = "<your-name>"
repo = "https://github.com/owner/repo"

# SKILL.md inside a subdirectory of a repo
[[skill.git_subpath]]
name    = "<your-name>"
repo    = "https://github.com/owner/repo"
subpath = "path/to/skill"

# Every subdirectory of a path becomes a skill (each must have SKILL.md)
[[skill.git_bulk]]
prefix  = "<short-prefix->"   # to namespace and avoid collisions
repo    = "https://github.com/owner/repo"
subpath = "skills"
```

Then `./skills/install.sh` (or `--force` if a same-named skill is already installed). Done.

To add a custom skill written in this repo:

```bash
mkdir -p skills/local/<your-name>
$EDITOR skills/local/<your-name>/SKILL.md
```

Then add to `manifest.toml`:

```toml
[[skill.local]]
name = "<your-name>"
src  = "local/<your-name>"
```

And re-run `./skills/install.sh`.

## Why not vendor every third-party skill directly

Vendoring upstream skills in this repo would force a re-vendor every time those skills update, plus they carry their own MIT/Apache licenses that should be respected at their source. The clone-on-install pattern keeps the repo small, the upstream attribution intact, and the install fresh. The custom skill (`plonky3-friendly-rust`) is vendored because it lives nowhere else.

## Why the plugin steps are manual

`/plugin marketplace add` and `/plugin install` are interactive Claude Code slash commands. The harness intentionally blocks an agent (this Claude session, or any other) from writing to `~/.claude/settings.json` to prevent self-modification of agent configuration. Running the slash commands at the prompt registers the marketplace and enables the plugin in the same file the harness would have written, just authorised by the user instead of by the agent.

## Licensing

The custom skill `plonky3-friendly-rust` is Apache-2.0 (matches the workspace). All third-party skills retain their upstream licenses; see the source repos linked above. The installer copies upstream content into `~/.claude/skills/` on the user's machine; nothing is redistributed by this repo.
