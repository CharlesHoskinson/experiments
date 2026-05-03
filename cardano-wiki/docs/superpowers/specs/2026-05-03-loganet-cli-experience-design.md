---
date: 2026-05-03
kind: design-doc
topic: LoganNet CLI experience — colors, ASCII art, banners, dashboard, progress UI
status: drafted (v3.1)
revisions:
  - v1: initial pass — three tiers, banner blink, per-event tail
  - v2: removed blink + emoji + per-event tail; collapsed to two tiers; per-role aggregate
  - v3: added widths + themes + sparklines + alarm row; sigil banner replaced portrait
  - v3.1: reverted sigil banner — proper lobster restored; goblin strip restored to dashboard
---

# Design: LoganNet CLI experience (v3.1)

## Why

`omega-experiment`, `omega-toy-consensus run`, and `omega-goblins run` are how every developer touches LoganNet. The repo has a strong painted visual identity — the CryptoLobster Logan logo and the six-goblin hero illustration — and the terminal experience needs to carry that energy without falling into emoji-soup or off-by-one column noise.

This spec defines the terminal-side identity: a recognisable Logan banner, a six-goblin status strip, a fixed-height dashboard with sparklines and an alarm row, color themes including a deuteranopia-safe palette, and a two-phase drain on `q` for graceful shutdown. Three rendering tiers collapsed to two (`pretty` / `plain`) plus a `pipe` mode for non-TTY output. One shared crate (`omega-tui`) underpins all three binaries.

## Principles

1. **Logan reads as a lobster, not a sigil.** A lobster portrait is recognisable in monospace ASCII when you give it a clear silhouette: raised claws holding emblems, antennae sweeping up, segmented carapace, splayed tail. Abstracting the body into geometric blocks ("CryptoLobster crest") was a regression — it lost the creature.
2. **Goblins live on the dashboard.** A 5-row strip of all six role glyphs is always visible during `omega-goblins run`, with inactive roles rendered greyed out so the layout never shifts.
3. **Fixed-height layouts everywhere.** No scroll, no jitter, no off-by-one column drift. Every component pins its height in advance.
4. **One emoji exception.** The alarm row is allowed `🚨` and `⚠` because attracting eyes is the row's only job. Everywhere else the rich tier uses 1-cell Unicode geometry; the plain tier uses pure ASCII.
5. **Pretty is opt-in for one-shots, default for daemons.** A user typing `omega-experiment submit` 50 times in a script doesn't want a 14-line banner each time. The banner shows up for `--style pretty` on one-shots, and by default for `omega-toy-consensus run` and `omega-goblins run`.

## Decisions

### Crate

- New workspace crate `crates/omega-tui/` with five modules: `tier`, `palette`, `banners/`, `goblins`, `style`, `progress`, `dashboard/`, `keybinds`.
- Every binary (`omega-experiment`, `omega-toy-consensus`, `omega-goblins-runner`) imports from `omega-tui` only.

### External deps (workspace.dependencies, consumed via workspace = true)

| Crate | Purpose |
|---|---|
| `owo-colors = "4"` | Colorising; auto-detects `NO_COLOR` and non-TTY |
| `indicatif = "0.18"` | Progress bars + spinners + multi-bar layouts |
| `comfy-table = "7"` | State / metrics tables; auto terminal-width |
| `crossterm = "0.28"` | Terminal capability detection (color depth, width, height, TTY-ness, raw mode, signal handling) |
| `insta = "1.43"` (dev-dep) | Snapshot tests |
| `proptest` (dev-dep, already in workspace) | Width / glyph invariant property tests |

`std::io::IsTerminal` (stable since 1.70) handles atty detection.

### Tiers and widths

```
                ┌─ Tier ───────────────────────────────────────────┐
                │                                                   │
                │   pretty   = full color + Unicode + animations    │
                │   plain    = no color, ASCII-only, no animation   │
                │                                                   │
                └───────────────────────────────────────────────────┘

                ┌─ TermWidth (only meaningful in pretty) ──────────┐
                │                                                   │
                │   compact  =  cols  <  80   (32-col banner)       │
                │   default  =  80 ≤ cols < 120  (60-col banner)    │
                │   wide     =  cols ≥ 120  (96-col banner)         │
                │                                                   │
                └───────────────────────────────────────────────────┘

                pipe mode = stdout is not a TTY → tier=plain forced;
                            no banner, no spinner, no key handling
```

Detection order, env var beats all:

1. `OMEGA_CLI_TIER=pretty|plain` env var sets the tier explicitly.
2. `--style pretty|plain` flag sets the tier explicitly.
3. `NO_COLOR` env var → plain.
4. `!stdout().is_terminal()` → plain (pipe mode is a special case of plain).
5. `available_color_count() < 256` → plain.
6. `LANG` does not contain `UTF-8` → plain.
7. otherwise → pretty.

`OMEGA_CLI_THEME=default|highcontrast|monochrome|deuteranopia` selects the palette. Default is parchment-and-cyan to match the painted assets.

### Color palette (default theme)

| Token | sRGB | Use |
|---|---|---|
| `loganet.deep` | `#2a3b6e` | Headers, borders, brand wordmark |
| `loganet.glow` | `#4ec3e8` | Cyan accents, success markers, LGN coin |
| `loganet.warm` | `#d4a850` | Banner highlights, cheliped tips |
| `loganet.parchment` | `#e8dcc4` | Background tint hint when supported |
| `loganet.error` | `#c84a3a` | Error markers |
| `loganet.muted` | `#8a8a8a` | Secondary text, timestamps, inactive goblins |

`highcontrast`: white-on-black + bright primaries. `monochrome`: dim/bold only, no color. `deuteranopia`: cyan-orange instead of green-red so the palette is colorblind-safe.

### Logan banner (default 60-col variant)

```
                          ╲ ╲       ╱ ╱
                           ╲ ╲     ╱ ╱
                            ╲ ╲   ╱ ╱
              ╔═════╗        ╲ ╲ ╱ ╱        ╔═════╗
              ║  ⬢  ║         ╲ ╳ ╱         ║  ⬡  ║
              ╚══╤══╝          ╲╱           ╚══╤══╝
                 ╲           ◉ ── ◉           ╱
                  ╲         ╱  ──  ╲         ╱
                   ╲═══════▕ ╔════╗ ▏═══════╱
                            ▕║▓▓▓▓║▏
                          ╔══╝▓▓▓▓╚══╗
                          ║▓▓▓░░▓▓░▓▓║
                          ╚╗▓░▓▓▓▓░▓╔╝
                           ╲╲▓▓▓▓▓▓╱╱
                            ╲╲▓▓▓▓╱╱
                             ╲────╱
                            ╱──────╲
                           ╱  ╱  ╲  ╲
                          ▟  ▟    ▙  ▙

         ━━━ LoganNet · v0.11.0 · LGN unit ━━━
```

Reads as a lobster on first glance: two antennae sweeping in (rows 1-3), two raised claws holding framed emblems `⬢` (Cardano) and `⬡` (LGN) (rows 4-6), eye-stalks (`◉ ── ◉`) above the mouth (rows 7-8), the cheliped arms framing into the body (row 9), the segmented carapace in `▓░▓` plates (rows 10-13), the splayed tail fan unfolding into rearmost flippers (rows 14-19), brand wordmark below.

`compact` 32-col variant strips antennae and tail fan to fit a tmux split-pane:

```
       ◉━━━◉
   ╔═══╗   ╔═══╗
   ║ ⬢ ║   ║ ⬡ ║
   ╚═══╝═══╚═══╝
       ╲▓▓╱
   LoganNet · v0.11
```

`wide` 96-col variant adds the same content with extra antennae detail and a tagline.

### Goblin glyphs (6 × 5×9 cells, no emoji)

```
   Holder      Whale       Adversary   Lurker      SnapServer  Validator
   ╭───╮       ╭═══╮       ╭───╮       ╭───╮       ╭───╮       ╭▓▓▓╮
   │ ◉ │       │◉◉◉│       │ ⊙ │       │ ◔ │       │ ◉ │       │ ◉ │
   ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯
   ─┤◉├─       ─┤◇├─       ─┤⚔├─       ─┤?├─       ─┤≡├─       ─┤△├─
    ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲
```

Each glyph is byte-exactly 5 rows × 9 cols in pretty tier. Plain tier replaces box-drawing with `+`/`-`/`|` and held props with letters (`o` Holder, `<>` Whale, `X` Adversary, `?` Lurker, `=` SnapServer, `^` Validator). Per-role color from the active `Palette`. When a role's count is zero, the glyph renders in `loganet.muted` so the dashboard layout never shifts.

### Goblin-run dashboard (fixed height ~22 rows)

```
┌─ LoganNet · t=00:01:42 / 00:30:00 ────────────────────────────────────────────┐
│                                                                                │
│   ⚠ apply p95 = 6.2s — slow                                  (3s ago)          │
│                                                                                │
│   Holder      Whale      Adversary    Lurker     SnapServer  Validator         │
│   ╭───╮       ╭═══╮       ╭───╮       ╭───╮       ╭───╮       ╭▓▓▓╮            │
│   │ ◉ │       │◉◉◉│       │ ⊙ │       │ ◔ │       │ ◉ │       │ ◉ │            │
│   ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯       ╰─┬─╯            │
│   ─┤◉├─       ─┤◇├─       ─┤⚔├─       ─┤?├─       ─┤≡├─       ─┤△├─            │
│    ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲         ╱ ╲             │
│                                                                                │
│   role          count  ticks  accept  reject   tick rate (1m)                  │
│   ────────────  ─────  ─────  ──────  ──────   ────────────────                │
│   Holder            5     87      86       1   ▁▂▃▄▅▆▇█▇▇▇▆▇█▇▇▆               │
│   Whale             1      6       6       0   ▁  ▁    ▁    ▁                  │
│   Adversary         2     14       0      14   ▂▂▃▃▂▂▂▃▃▂▂▂▃▂▃▃                │
│   Lurker            1     12      12       0   ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁                │
│   SnapServer        1     28      28       0   ▂▂▂▃▂▂▂▂▂▂▂▂▂▂▂▂                │
│   Validator         0      0       0       0                                   │
│                                                                                │
│   cluster: leader=node-2  term=4  log-tip=93  applied=93                       │
│   ledger:  nullifiers=287  starstream-utxos=287  WAL=12 MiB                    │
│                                                                                │
│   q drain  ^C abort                                                            │
└────────────────────────────────────────────────────────────────────────────────┘
```

Sparklines `▁▂▃▄▅▆▇█` are 1-cell-wide on every monospace font. 16-bucket history at 1s buckets = 16s rolling window; UI updates every 1s.

### Alarm row triggers

| Trigger | Text | Color | Sticky |
|---|---|---|---|
| Adversary submission *accepted* | `🚨 ADVERSARY ACCEPTED — see log idx N` | red on yellow | 5s, replaceable |
| Cluster lost quorum | `🚨 QUORUM LOST — leader=none` | red on yellow | until quorum restored |
| Apply p95 > 5s over last 60s | `⚠ apply p95 = X.Ys — slow` | yellow | 5s |
| LLM JSON parse failure rate > 10% / 1m | `⚠ LLM JSON drift — N%` | yellow | 5s |
| No alarm | row blank | — | n/a |

The alarm row is the *only* place emoji is used; documented exception in code.

### One-shot command UI

`omega-experiment` and other one-shot commands default to `--style plain --no-banner`. Long-running operations within a one-shot (e.g. `prove`) show a tier-aware spinner with a single status line, replaced by `✔` / `✘` / `[ok]` / `[err]` on completion. `--style pretty` opt-in shows the banner first.

```
[ok]   build genesis     256-leaf synthetic UTxO            0.4s
[ok]   start node 1      127.0.0.1:4001 / api 8001          1.2s
[ok]   prove leaf #42                                        3.4s
[ok]   submit            via leader=node 2                   0.2s
[ok]   apply             log_idx=12 (3/3 nodes)              0.1s
[done] round trip complete · LGN UTxO 1234.5678
```

### Keybind contract

| Key | Behaviour |
|---|---|
| `q` / `Q` | Begin two-phase graceful drain; only valid in long-running daemons |
| `^C` | Immediate abort, exit 130 |
| `^Z` | Suspend (raw mode dropped, SIGTSTP raised, re-enter raw mode on SIGCONT) |
| `^\` | Quit + core dump (default OS handler, not intercepted) |
| any other | Ignored, no echo |

**Two-phase drain**:

```
                    user presses q
                           │
                           ▼
                  ┌──────────────────┐
                  │ phase 1 (immediate)
                  │ • shutting_down=true
                  │ • no new ticks
                  │ • footer flips:
                  │   "draining N goblins..."
                  └────────┬─────────┘
                           │
                           │  5s budget
                           ▼
                  ┌──────────────────┐
                  │ phase 2 (race)
                  │ • per-goblin tick
                  │   races against deadline
                  │ • countdown footer:
                  │   "draining N... 3s remaining"
                  └────────┬─────────┘
                           │
                           ▼
                       exit 0
```

In `pipe` mode (no TTY) no key handler is registered.

### Capability + theme detection (single function)

```rust
pub struct Tier { pub pretty: bool, pub width: TermWidth }
pub enum TermWidth { Compact, Default, Wide }
pub enum Theme { Default, HighContrast, Monochrome, Deuteranopia }

pub fn detect() -> (Tier, Theme) {
    let pretty = match env_var("OMEGA_CLI_TIER").as_deref() {
        Ok("pretty") => true,
        Ok("plain")  => false,
        _ => env_var("NO_COLOR").is_err()
            && std::io::stdout().is_terminal()
            && available_color_count() >= 256
            && env_var("LANG").map_or(false, |s| s.contains("UTF-8")),
    };
    let cols = crossterm::terminal::size().map(|(c, _)| c).unwrap_or(80);
    let width = if cols < 80 { TermWidth::Compact }
                else if cols >= 120 { TermWidth::Wide }
                else { TermWidth::Default };
    let theme = env_var("OMEGA_CLI_THEME").as_deref()
        .map(parse_theme).unwrap_or(Theme::Default);
    (Tier { pretty, width }, theme)
}
```

CLI flags `--style`, `--width`, `--theme` override env vars at the per-binary level. Documented in every `--help`.

### File layout

```
crates/omega-tui/
├── Cargo.toml
├── src/
│   ├── lib.rs              detect() + re-exports
│   ├── tier.rs             Tier + TermWidth + detect()
│   ├── palette.rs          4 themes; Palette struct
│   ├── banners/
│   │   ├── mod.rs
│   │   ├── compact.rs      32-col Logan
│   │   ├── default.rs      60-col Logan (the v3.1 lobster)
│   │   └── wide.rs         96-col Logan
│   ├── goblins.rs          six 5×9 role glyphs (pretty + plain)
│   ├── style.rs            color tokens + ✔✘⚠▶ glyphs + ok/err/warn helpers
│   ├── progress.rs         tier-aware indicatif spinner + progress bar
│   ├── dashboard/
│   │   ├── mod.rs          fixed-height goblin-run dashboard
│   │   ├── strip.rs        the 5-row goblin strip (greyed when count=0)
│   │   ├── sparkline.rs    ▁▂▃▄▅▆▇█ 16-bucket compress
│   │   └── alarm.rs        single-slot sticky alarm row
│   └── keybinds.rs         q/^C/^Z + two-phase drain machine
├── tests/
│   ├── snapshots.rs        insta × tier × width × theme
│   ├── tier_detection.rs   env-var + flag matrix
│   ├── width_invariants.rs banner widths bounded
│   ├── glyph_invariants.rs every goblin 5×9; greyed-out variant tested
│   ├── lf_normalisation.rs CRLF/LF agnostic snapshots
│   └── drain_phases.rs     simulated drain countdown
└── README.md
```

### Snapshot test matrix

| tier × width × theme | rows committed |
|---|---|
| pretty × compact × {default, highcontrast, deuteranopia} | 3 banner snapshots |
| pretty × default × {default, highcontrast, deuteranopia} | 3 banner snapshots |
| pretty × wide × {default, highcontrast, deuteranopia} | 3 banner snapshots |
| plain × — × {default, highcontrast, monochrome} | 3 banner snapshots |
| pretty × default × default | 6 goblin snapshots (one per role) |
| plain × — × default | 6 goblin snapshots |
| pretty × default × default | 1 dashboard snapshot |
| plain × — × default | 1 dashboard snapshot |

Total: 26 committed snapshots. CI matrix runs on Linux + macOS + Windows, LF-normalised before compare.

## Acceptance gates

- v3.1 Logan banner reads as a lobster on first glance — three width variants tested (compact, default, wide).
- All six goblin glyphs are byte-exactly 5 rows × 9 cols across all four themes; greyed-out variant tested.
- Goblin-run dashboard is fixed-height (~22 rows); no scroll, no jitter.
- Sparklines render at 16-bucket × 1s resolution.
- Alarm row is the only emoji surface; sticky 5s; replaceable by higher-severity alarms.
- `OMEGA_CLI_TIER` + `OMEGA_CLI_THEME` env vars beat flags beat auto-detection.
- Two-phase drain on `q` with 5s budget and countdown footer.
- One-shot commands default to plain + spinner; banner opt-in via `--style pretty`.
- CI matrix runs `--print-banner` snapshot diff on Linux + macOS + Windows × pretty + plain × default + highcontrast.
- All snapshots LF-normalised; no CRLF noise.
- `cargo test --package omega-tui --no-fail-fast` green.

## Out of scope

- TUI / curses / alternate-screen-buffer.
- JSON output redesign — `--json` and `--style pipe` continue to emit machine-friendly output.
- Brand redesign of the painted assets.
- Asciinema recording for the README (deferred to follow-up commit).
