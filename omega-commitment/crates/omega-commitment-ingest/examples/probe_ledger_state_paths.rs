//! Probe the documented JSON paths in a `cardano-cli conway query ledger-state`
//! dump — proves Tasks 7 (stake) and 8 (governance) can navigate the real
//! mainnet output without OOM or shape surprises.
//!
//! Exercise:
//!   cargo run --release -p omega-commitment-ingest --example probe_ledger_state_paths -- \
//!     ~/cardano/snapshots/ledger_state_*.json
//!
//! Reports:
//!   path | type | len | sample_key | wall_time_to_reach
//! plus peak RSS (VmHWM) at the end.

use serde_json::Value;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

const STAKE_GOV_PATHS: &[&str] = &[
    // stake sub-tree
    "stateBefore.esLState.delegationState.dstate.accounts",
    "stateBefore.esLState.delegationState.dstate.genDelegs",
    "stateBefore.esLState.delegationState.pstate.stakePools",
    "stateBefore.esLState.utxoState.stake.credentials",
    "stateBefore.esSnapshots.pstakeMark.activeStake",
    "stateBefore.esSnapshots.pstakeMark.stakePoolsSnapShot",
    "stateBefore.esSnapshots.pstakeSet.activeStake",
    "stateBefore.esSnapshots.pstakeGo.activeStake",
    // governance sub-tree
    "stateBefore.esLState.delegationState.vstate.dreps",
    "stateBefore.esLState.delegationState.vstate.committeeState",
    "stateBefore.esLState.utxoState.ppups",
    "stateBefore.esLState.utxoState.ppups.proposals",
    "stateBefore.esLState.utxoState.ppups.committee",
    "stateBefore.esLState.utxoState.ppups.constitution",
    "stateBefore.esLState.utxoState.ppups.currentPParams",
    // chain account state
    "stateBefore.esChainAccountState.reserves",
    "stateBefore.esChainAccountState.treasury",
    // explicit absences (UTxO must be empty — proves the cli scrub)
    "stateBefore.esLState.utxoState.utxo",
];

fn navigate<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn shape(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(_) => "number".into(),
        Value::String(_) => "string".into(),
        Value::Array(a) => format!("array len={}", a.len()),
        Value::Object(o) => format!("object len={}", o.len()),
    }
}

fn sample_key(v: &Value) -> String {
    match v {
        Value::Object(o) => o.keys().next().cloned().unwrap_or_else(|| "<empty>".into()),
        Value::Array(a) => a
            .first()
            .map(|x| x.to_string())
            .unwrap_or_else(|| "<empty>".into()),
        Value::String(s) => s.chars().take(40).collect(),
        Value::Number(n) => n.to_string(),
        other => format!("{other:?}").chars().take(40).collect(),
    }
}

fn read_vmhwm() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

fn main() -> anyhow::Result<()> {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: probe_ledger_state_paths <path-to-ledger.json>");
            std::process::exit(2);
        }
    };
    eprintln!("probe: opening {path}");
    let started = Instant::now();
    let file = File::open(&path)?;
    let bytes = file.metadata()?.len();
    let reader = BufReader::with_capacity(8 * 1024 * 1024, file);
    eprintln!(
        "probe: file size {} bytes ({:.2} GiB)",
        bytes,
        bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    let load_started = Instant::now();
    let root: Value = serde_json::from_reader(reader)?;
    let load_ms = load_started.elapsed().as_millis();
    let after_load_rss = read_vmhwm();
    eprintln!(
        "probe: serde_json::from_reader → Value done in {load_ms} ms, peak RSS so far {} KiB ({:.2} GiB)",
        after_load_rss,
        after_load_rss as f64 / (1024.0 * 1024.0)
    );

    println!();
    println!("{:<70} | {:<22} | sample", "path", "shape");
    println!("{:-<70}-+-{:-<22}-+-{:-<40}", "", "", "");

    for &p in STAKE_GOV_PATHS {
        match navigate(&root, p) {
            Some(v) => println!("{p:<70} | {:<22} | {}", shape(v), sample_key(v)),
            None => println!("{p:<70} | {:<22} | (path not present)", "MISSING"),
        }
    }

    let final_rss = read_vmhwm();
    let total_ms = started.elapsed().as_millis();
    eprintln!();
    eprintln!(
        "probe: total wall {total_ms} ms ; peak VmHWM {} KiB ({:.2} GiB)",
        final_rss,
        final_rss as f64 / (1024.0 * 1024.0)
    );
    eprintln!(
        "probe: ratio peak_RSS / file_size = {:.2}x",
        final_rss as f64 * 1024.0 / bytes as f64
    );
    Ok(())
}
