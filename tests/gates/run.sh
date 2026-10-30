#!/usr/bin/env bash
# tests/gates/run.sh: the one entry point for the local verification gates of spark-rsi.
# The host test gate builds its fixture executable and runs the host suite; GitHub-hosted
# checks build only the pinned verifier. Manifests under tests/gates/ name this script plus
# one step word, so no manifest needs shell expansion (ADR 0028 P6). Observations print as
# "AIEN-OBS <name> <json>".
#
# Sibling checkout: Cargo.toml depends on ../aien-protocols/crates/* by path.
# Provide an isolated aien-dev/aien-protocols-mv checkout under that sibling name.
#
# Isolation prerequisites are observed, never provisioned here. The gate does not change
# AppArmor, setuid bits, user-manager configuration or cgroup delegation.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
step="${1:?usage: tests/gates/run.sh <step>}"
obs() { printf 'AIEN-OBS %s %s\n' "$1" "$2"; }
need_protocols() {
  if [ ! -d ../aien-protocols/crates ]; then
    echo "run.sh: ../aien-protocols is missing; the hosted job checks out aien-dev/aien-protocols as a sibling" >&2
    exit 3
  fi
  obs sibling_protocols_commit "\"$(git -C ../aien-protocols rev-parse HEAD 2>/dev/null || echo unknown)\""
}

case "$step" in
  crumb-verify)
    "${CRUMB:-crumb}" verify .
    ;;
  fmt)
    need_protocols
    cargo fmt --all -- --check
    ;;
  check)
    need_protocols
    cargo check --offline --locked -j2 --all-targets
    ;;
  test)
    need_protocols
    obs bwrap_present "$(command -v bwrap >/dev/null && echo true || echo false)"
    obs apparmor_restrict_userns "\"$(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || echo unknown)\""
    obs bwrap_setuid "$( [ -u /usr/bin/bwrap ] && echo true || echo false)"
    # Build a local fixture executable, never a shared candidate artifact cache.
    env -u CARGO_TARGET_DIR cargo build --offline --locked --release -j2
    obs prepopulate_rc 0
    # Keep the in-tree binary location expected by the judge unit tests.
    # prepopulate_rc is the existing gate observation for this fixture build.
    env -u CARGO_TARGET_DIR CARGO_BUILD_JOBS=2 cargo test --offline --locked -j2 --verbose -- --test-threads=1
    ;;
  zero-disk-secrets)
    # Not a hosted step in this repository; the same invariant the sibling repos enforce.
    echo "Auditing repository for prohibited plaintext secret files..."
    if find . -maxdepth 4 -name ".env*" -not -path "*/.git/*" -not -path "*/target/*" | grep -q .; then
      echo "::error::Prohibited plaintext .env file found. Use an approved AIEN SecretProvider."
      obs env_files_found true
      exit 1
    fi
    obs env_files_found false
    echo "Zero plaintext .env files verified."
    ;;
  *)
    echo "run.sh: unknown step '$step'" >&2
    exit 2
    ;;
esac
