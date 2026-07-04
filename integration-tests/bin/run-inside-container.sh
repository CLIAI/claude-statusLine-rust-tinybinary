#!/usr/bin/env bash
set -euo pipefail

REPO=/work/repo
BIN="${REPO}/target/release/claude-statusline-rust-tinybinary"
ARTIFACT_DIR="${REPO}/integration-tests/tmp"

mkdir -p "${ARTIFACT_DIR}"
cd "${REPO}"

export PATH="${REPO}/target/release:/root/.cargo/bin:/root/.local/bin:/root/.claude/local:/root/.claude/local/bin:${PATH}"
export HOME=/root
export TERM=xterm-256color

log() {
  printf '==> %s\n' "$*"
}

have_claude() {
  command -v claude >/dev/null 2>&1
}

capture_pane() {
  local session=$1
  local output=$2
  tmux capture-pane -t "${session}:0.0" -p -S -200 >"${output}" 2>/dev/null || true
}

has_debug_log() {
  local dir=$1
  find "${dir}" -type f -name '*.jsonl' -print -quit | grep -q .
}

drive_onboarding_if_needed() {
  local session=$1
  local capture=$2

  if grep -q 'Choose the text style' "${capture}"; then
    tmux send-keys -t "${session}" Enter
    return 0
  fi

  if grep -q 'Do you want to use this API key' "${capture}"; then
    tmux send-keys -t "${session}" 1 Enter
    return 0
  fi

  return 1
}

run_case() {
  local name=$1
  local cols=$2
  local rows=$3
  local expected=$4
  shift 4

  local session="sl_${name}"
  local debug_dir="${ARTIFACT_DIR}/debug-${name}"
  local capture="${ARTIFACT_DIR}/capture-${name}-${cols}x${rows}.txt"
  local settings="${HOME}/.claude/settings.json"

  rm -rf "${debug_dir}"
  mkdir -p "${debug_dir}" "${HOME}/.claude"
  tmux kill-session -t "${session}" >/dev/null 2>&1 || true

  python3 "${REPO}/add-to-claude-settings.py" \
    --settings "${settings}" \
    --binary "${BIN}" \
    --debug-log-dir "${debug_dir}" \
    "$@"

  tmux new-session -d -x "${cols}" -y "${rows}" -s "${session}" \
    "cd ${REPO}; claude; rc=\$?; echo CLAUDE_EXIT:\${rc}; sleep 3600"

  local ok=0
  for _ in $(seq 1 30); do
    sleep 1
    capture_pane "${session}" "${capture}"
    drive_onboarding_if_needed "${session}" "${capture}" || true

    if has_debug_log "${debug_dir}" && grep -Eq "${expected}" "${capture}"; then
      ok=1
      break
    fi

    if grep -q 'Select login method' "${capture}"; then
      break
    fi
  done

  capture_pane "${session}" "${capture}"
  tmux kill-session -t "${session}" >/dev/null 2>&1 || true

  if [[ "${ok}" != "1" ]]; then
    {
      echo "case=${name}"
      echo "size=${cols}x${rows}"
      echo "expected=${expected}"
      echo "settings=${settings}"
      echo
      echo "Claude Code did not display or invoke the configured statusLine."
      echo "The test requires Claude Code to reach the interactive prompt."
      echo "If the capture shows an auth or login prompt, pass the required auth into Docker with INTEGRATION_TESTS_DOCKER_ARGS."
    } >"${ARTIFACT_DIR}/failure-${name}.txt"
    return 1
  fi

  log "passed ${name} at ${cols}x${rows}"
}

log "Checking Claude Code CLI"
if ! have_claude; then
  echo "claude CLI was not installed by the Docker image" >&2
  exit 1
fi
claude --version | tee "${ARTIFACT_DIR}/claude-version.txt"

log "Compiling repository"
cargo build --release
cargo test

log "Running claude update"
claude update | tee "${ARTIFACT_DIR}/claude-update.txt"

log "Running tmux statusLine scenarios"
run_case default_80x24 80 24 'ctx|week|Claude|status' --style default
run_case full_compact_120x32 120 32 '[|]|Claude|status' --style full --compact
run_case weekly_narrow_60x18 60 18 'ctx|week|Claude|status' --style weekly
run_case custom_100x28 100 28 'SLIT-custom' --format 'SLIT-custom:%M|%w|%r|%C|%c'

log "All tmux integration scenarios passed"
