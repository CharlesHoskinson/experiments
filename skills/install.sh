#!/usr/bin/env bash
# install.sh — reproduce the Claude Code skill + tooling setup for this repo.
#
# Idempotent: safe to re-run. Existing skills are left in place unless
# `--force` is passed. Reads skills/manifest.toml as the source of truth.
#
# Usage:
#   ./skills/install.sh              # install everything declared in manifest
#   ./skills/install.sh --dry-run    # print what would happen, do nothing
#   ./skills/install.sh --force      # overwrite existing skills
#   ./skills/install.sh --skills     # skills only, skip cargo + rustup
#   ./skills/install.sh --cargo      # cargo binaries only
#   ./skills/install.sh --rustup     # rustup components only
#   ./skills/install.sh --verify     # check what is installed and exit
#
# Requirements on the host:
#   - bash 4+, git, rsync (or cp), curl
#   - rustup + cargo (https://rustup.rs)
#   - claude code CLI (~/.claude exists)

set -euo pipefail

SKILLS_DIR="${HOME}/.claude/skills"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${REPO_ROOT}/skills/manifest.toml"
LOCAL_DIR="${REPO_ROOT}/skills/local"
TMP_DIR="$(mktemp -d -t skill-install.XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

DRY_RUN=0
FORCE=0
DO_SKILLS=1
DO_CARGO=1
DO_RUSTUP=1
DO_VERIFY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --force)   FORCE=1 ;;
    --skills)  DO_CARGO=0; DO_RUSTUP=0 ;;
    --cargo)   DO_SKILLS=0; DO_RUSTUP=0 ;;
    --rustup)  DO_SKILLS=0; DO_CARGO=0 ;;
    --verify)  DO_VERIFY=1 ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
  shift
done

log()  { echo "[install.sh] $*"; }
warn() { echo "[install.sh] WARN: $*" >&2; }
die()  { echo "[install.sh] ERROR: $*" >&2; exit 1; }
run()  { if (( DRY_RUN )); then echo "  + $*"; else "$@"; fi; }

# ---------------------------------------------------------------------------
# Manifest parser (single-file embedded TOML reader; intentionally small)
# Emits one record per line in the form:
#   skill_local       <name> <src>
#   skill_git_root    <name> <repo>
#   skill_git_subpath <name> <repo> <subpath>
#   skill_git_bulk    <prefix> <repo> <subpath>
#   plugin            <name> <marketplace>
#   cargo_install     <name> [<post-install command>]
#   rustup_component  <toolchain> <comma,separated,components>
# ---------------------------------------------------------------------------

parse_manifest() {
  python3 - "${MANIFEST}" <<'PY'
import sys, tomllib
data = tomllib.loads(open(sys.argv[1], "rb").read().decode())
for entry in data.get("skill", {}).get("local", []):
    print(f"skill_local\t{entry['name']}\t{entry['src']}")
for entry in data.get("skill", {}).get("git_root", []):
    print(f"skill_git_root\t{entry['name']}\t{entry['repo']}")
for entry in data.get("skill", {}).get("git_subpath", []):
    print(f"skill_git_subpath\t{entry['name']}\t{entry['repo']}\t{entry['subpath']}")
for entry in data.get("skill", {}).get("git_bulk", []):
    print(f"skill_git_bulk\t{entry['prefix']}\t{entry['repo']}\t{entry['subpath']}")
for entry in data.get("plugin", []):
    print(f"plugin\t{entry['name']}\t{entry['marketplace']}")
for entry in data.get("cargo_install", []):
    post = entry.get("post-install", "")
    print(f"cargo_install\t{entry['name']}\t{post}")
for entry in data.get("rustup_component", []):
    print(f"rustup_component\t{entry['toolchain']}\t{','.join(entry['components'])}")
PY
}

# ---------------------------------------------------------------------------
# Skill installers
# ---------------------------------------------------------------------------

skill_target() { echo "${SKILLS_DIR}/$1"; }

skill_exists() { [[ -d "$(skill_target "$1")" ]]; }

ensure_clone() {
  local repo="$1" dest="$2"
  if [[ ! -d "${dest}" ]]; then
    run git clone --depth 1 --quiet "${repo}" "${dest}"
  fi
}

install_skill_local() {
  local name="$1" src="$2"
  local target; target="$(skill_target "${name}")"
  if skill_exists "${name}" && (( ! FORCE )); then
    log "skill (local) '${name}' already present, skipping"
    return
  fi
  log "skill (local) '${name}' from ${src}"
  run rm -rf "${target}"
  run cp -r "${REPO_ROOT}/skills/${src}" "${target}"
}

install_skill_git_root() {
  local name="$1" repo="$2"
  local target; target="$(skill_target "${name}")"
  if skill_exists "${name}" && (( ! FORCE )); then
    log "skill (git_root) '${name}' already present, skipping"
    return
  fi
  log "skill (git_root) '${name}' from ${repo}"
  local clone="${TMP_DIR}/$(echo "${repo}" | sed 's|.*/||; s|\.git$||')"
  ensure_clone "${repo}" "${clone}"
  run rm -rf "${target}"
  run cp -r "${clone}" "${target}"
}

install_skill_git_subpath() {
  local name="$1" repo="$2" subpath="$3"
  local target; target="$(skill_target "${name}")"
  if skill_exists "${name}" && (( ! FORCE )); then
    log "skill (git_subpath) '${name}' already present, skipping"
    return
  fi
  log "skill (git_subpath) '${name}' from ${repo}/${subpath}"
  local clone_name; clone_name="$(echo "${repo}" | sed 's|.*/||; s|\.git$||')"
  local clone="${TMP_DIR}/${clone_name}"
  ensure_clone "${repo}" "${clone}"
  if [[ ! -d "${clone}/${subpath}" ]]; then
    warn "subpath '${subpath}' not found in ${repo}; skipping"
    return
  fi
  run rm -rf "${target}"
  run cp -r "${clone}/${subpath}" "${target}"
}

install_skill_git_bulk() {
  local prefix="$1" repo="$2" subpath="$3"
  log "skill (git_bulk) prefix='${prefix}' from ${repo}/${subpath}"
  local clone_name; clone_name="$(echo "${repo}" | sed 's|.*/||; s|\.git$||')"
  local clone="${TMP_DIR}/${clone_name}"
  ensure_clone "${repo}" "${clone}"
  if [[ ! -d "${clone}/${subpath}" ]]; then
    warn "subpath '${subpath}' not found in ${repo}; skipping"
    return
  fi
  for subdir in "${clone}/${subpath}"/*/; do
    [[ -d "${subdir}" ]] || continue
    local sub_name; sub_name="$(basename "${subdir}")"
    [[ -f "${subdir}/SKILL.md" ]] || continue
    local target_name="${prefix}${sub_name}"
    local target; target="$(skill_target "${target_name}")"
    if skill_exists "${target_name}" && (( ! FORCE )); then
      continue
    fi
    run rm -rf "${target}"
    run cp -r "${subdir}" "${target}"
  done
  log "  -> bulk-installed $(ls "${clone}/${subpath}" | wc -l) skills with prefix '${prefix}'"
}

# ---------------------------------------------------------------------------
# Cargo + rustup
# ---------------------------------------------------------------------------

cargo_bin_present() {
  command -v "$1" >/dev/null 2>&1 && return 0
  [[ -x "${HOME}/.cargo/bin/$1" ]] && return 0
  return 1
}

install_cargo() {
  local name="$1" post="$2"
  local bin
  case "${name}" in
    kani-verifier) bin="cargo-kani" ;;
    *) bin="${name}" ;;
  esac
  if cargo_bin_present "${bin}" && (( ! FORCE )); then
    log "cargo binary '${name}' already installed (${bin})"
  else
    log "cargo install ${name}"
    run cargo install "${name}"
  fi
  if [[ -n "${post}" ]]; then
    log "  + post-install: ${post}"
    run bash -c "${post}"
  fi
}

install_rustup() {
  local toolchain="$1" components_csv="$2"
  log "rustup toolchain='${toolchain}' components=${components_csv}"
  local installed
  installed="$(rustup toolchain list 2>/dev/null | awk '{print $1}')"
  if ! echo "${installed}" | grep -q "^${toolchain}-"; then
    if [[ "${toolchain}" == "nightly" ]] || [[ "${toolchain}" == "stable" ]]; then
      run rustup toolchain install "${toolchain}"
    else
      warn "toolchain '${toolchain}' not installed and not auto-installable; skipping"
      return
    fi
  fi
  IFS=',' read -ra comps <<<"${components_csv}"
  for c in "${comps[@]}"; do
    run rustup component add --toolchain "${toolchain}" "${c}" 2>&1 | tail -1
  done
}

# ---------------------------------------------------------------------------
# Verify
# ---------------------------------------------------------------------------

do_verify() {
  log "skills installed at ${SKILLS_DIR}:"
  while IFS=$'\t' read -ra parts; do
    case "${parts[0]}" in
      skill_local|skill_git_root|skill_git_subpath)
        local name="${parts[1]}"
        if skill_exists "${name}"; then echo "  ✓ ${name}"; else echo "  ✗ ${name} MISSING"; fi
        ;;
      skill_git_bulk)
        local prefix="${parts[1]}"
        local count
        count="$(ls -d "${SKILLS_DIR}/${prefix}"* 2>/dev/null | wc -l)"
        echo "  ${prefix}* count: ${count}"
        ;;
      cargo_install)
        local name="${parts[1]}"
        local bin="${name}"; [[ "${name}" == "kani-verifier" ]] && bin="cargo-kani"
        if cargo_bin_present "${bin}"; then echo "  ✓ cargo ${name}"; else echo "  ✗ cargo ${name} MISSING"; fi
        ;;
    esac
  done < <(parse_manifest)
  log "plugins (must be enabled inside Claude Code):"
  while IFS=$'\t' read -ra parts; do
    [[ "${parts[0]}" == "plugin" ]] || continue
    echo "  - ${parts[1]} (from ${parts[2]})"
  done < <(parse_manifest)
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if (( DO_VERIFY )); then
  do_verify
  exit 0
fi

mkdir -p "${SKILLS_DIR}"

while IFS=$'\t' read -ra parts; do
  kind="${parts[0]}"
  case "${kind}" in
    skill_local)
      (( DO_SKILLS )) && install_skill_local "${parts[1]}" "${parts[2]}"
      ;;
    skill_git_root)
      (( DO_SKILLS )) && install_skill_git_root "${parts[1]}" "${parts[2]}"
      ;;
    skill_git_subpath)
      (( DO_SKILLS )) && install_skill_git_subpath "${parts[1]}" "${parts[2]}" "${parts[3]}"
      ;;
    skill_git_bulk)
      (( DO_SKILLS )) && install_skill_git_bulk "${parts[1]}" "${parts[2]}" "${parts[3]}"
      ;;
    cargo_install)
      (( DO_CARGO )) && install_cargo "${parts[1]}" "${parts[2]:-}"
      ;;
    rustup_component)
      (( DO_RUSTUP )) && install_rustup "${parts[1]}" "${parts[2]}"
      ;;
  esac
done < <(parse_manifest)

cat <<'EOF'

Done with the script-installable parts.

Manual steps remaining (Claude Code plugins; the harness blocks programmatic
edits to settings.json, so these run inside Claude Code itself):

  /plugin marketplace add obra/superpowers-marketplace
  /plugin install superpowers@superpowers-marketplace

  /plugin marketplace add kfchou/wiki-skills
  /plugin install wiki-skills-v2@wiki-skills

After running them once, the marketplace + plugin choices persist in
~/.claude/settings.json across all future sessions.

To verify everything: ./skills/install.sh --verify
EOF
