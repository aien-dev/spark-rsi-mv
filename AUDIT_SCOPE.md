# Audit scope

Written for an outside, independent security audit. Revision described:
`fc13d3f77e6cfec789fce1b0e15125df8feac3d8` on `main`, 2026-10-26. Everything below is taken from the repository's own files at
that revision; items marked NOT_RUN were not executed while writing this document.

## What this repository is

A Rust crate `spark-rsi` 0.1.0 (AGPL-3.0-or-later) that runs a five-phase cycle against a target
repository: observe, propose (model proposals applied in a sandbox copy), verify (unslop check,
secret refusal, `cargo check`, `cargo test`), balance (`src/balance.rs`), ratify (review branch
`rsi/<proposal id>` plus a patch; a human merges). Binaries: `spark-rsi` (`src/main.rs`),
`spark-rsi-supervisor` (`src/bin/supervisor.rs`) and `spark-rsi-judge` (`src/actor/judge.rs`). It depends on
`aien-protocols-mv` crates by path, so it is not a standalone checkout.

## What to review, in priority order

1. `src/isolation/cargo_sandbox.rs`, `src/isolation/container.rs`, `src/isolation/rollback.rs`:
   the bubblewrap build jail, environment clearing, toolchain binding (`CARGO_HOME`, `RUSTUP_HOME`
   and the `PATH` directory are bound read-only; a directory that is or holds the judge's home is
   never exposed, per README), and the cgroup limits. `rollback.rs` holds 12 `Command::new` calls.
2. `src/supervisor/` (`canary_exec.rs`, `daemon.rs`, `ipc.rs`) and `src/canary/`: canary worker
   namespaces, the one IPC socket, the worker environment allowlist, and how supervisor evidence
   stays outside the jail. Contract: `docs/CANARY-CONTRACT.md`.
3. `src/ratify.rs` and `src/diff_gate.rs`: protected-file checks on the real git diff.
4. `src/promotion_gate.rs`, `src/actor/judge.rs`, `src/verifier.rs`: version 2 receipts
   (`docs/RECEIPT-V2.md`), judge policy and public key pins (`--judge-policy-file`,
   `--judge-public-key` and the `RSI_JUDGE_*` environment variables); malformed pins are meant to be
   refused at startup.
5. `src/ledger/` (`merkle.rs`, `db.rs`, `record.rs`): the improvement ledger and its signing keys
   (README: kept off the command line).
6. `src/propose/` (`model_seat.rs`, `cortex.rs`): model replies are untrusted input that becomes
   a candidate patch.
7. `src/observe.rs`, `src/measure/profile.rs`, `src/propose/kernel_autotune.rs`,
   `src/soak/runner.rs`: other process launches.

## Trust boundaries

- Untrusted: model output and every candidate patch, candidate build scripts and tests, canary
  worker output.
- Trusted: the supervisor and judge binaries, the judge policy file and public key, the ledger
  signing key, the host toolchain, bubblewrap and systemd.
- The promotion gate trusts only a receipt from a separate judge (README, `docs/RECEIPT-V2.md`).
- Defaults in `src/main.rs` point at loopback model and memory services on the reference host.

## Threat model summary

- Assets: the target repository's default branch, the host outside the jail, judge keys,
  ledger integrity, the honesty of evaluation receipts.
- Attackers considered: a hostile or wrong candidate patch, including one whose build script or
  tests try to read host files, inherit the service environment, write outside its output, or
  reach the network.
- Defended: bubblewrap jail with cleared environment and no network, cgroup limits, canary
  namespaces, branch-only ratification, separate judge receipts, `.env` refusal gate.
- Not defended / explicitly out of scope: kernel or bubblewrap escapes; a compromised judge key;
  hosts where unprivileged user namespaces are restricted (the gate only observes AppArmor and
  setuid settings, it does not change them).

## Build from source at a pin

Needs Rust/Cargo, bubblewrap, and `aien-protocols-mv` cloned next to this repository as
`aien-protocols`. README asks for the sibling at `28bc49ec57a89398fcb463cf71c0add7fcd2ef6c`
until aien-protocols-mv#59 merges; #59 merged as `f3785177fcaaab7c0f2ce1126616e9fecb4e47b4`, so
the default branch of aien-protocols-mv carries it. `Cargo.lock` committed (265 packages).

```sh
git clone https://github.com/aien-dev/spark-rsi-mv spark-rsi
git clone https://github.com/aien-dev/aien-protocols-mv aien-protocols
cd spark-rsi
cargo build --locked
```

## Run every test

NOT_RUN while writing this document. From README and `tests/gates/run.sh`:

```sh
cargo test --locked -- --test-threads=1
tests/gates/run.sh crumb-verify        # needs the crumb tool
tests/gates/run.sh zero-disk-secrets
tests/gates/run.sh fmt
tests/gates/run.sh check               # cargo check --offline --locked -j2 --all-targets
tests/gates/run.sh test                # release build, then cargo test --offline --locked
sh scripts/check-no-python.sh
```

The tree has 21 top-level test files under `tests/` and 340 `#[test]` or `#[tokio::test]` attributes in
`src/` and `tests/`. The jail tests need bubblewrap and a systemd user manager with cgroup v2.

## Known limitations and NOT_RUN

- README: pre-alpha, not part of the WHOLE-SYSTEM-E2E v1 release; no unattended
  self-modification and no automatic promotion.
- `evidence/rsi-cut-2026-10-12/REPORT.md` qualifies the canary mechanism only, not
  self-improvement (README).
- Open issues named in README: #54 (qualify one independent version 2 evaluation and review
  branch cycle) and #57.
- Receipts and plan digests produced before the aien-protocols#59 cutover do not verify (README).
- The tracked workflow's header comment says nothing on GitHub gates a merge, while README calls
  `AIEN Verification` the one required check. Branch protection is not visible from the
  repository.

## Dependencies and provenance

- `Cargo.toml` lists 22 direct dependencies, including clap, tokio, reqwest 0.12, sha2, uuid,
  libc, and the path crates `spark-crumbs` and `crumb-spec`. README: rustls moved to 0.23.46 for
  four security advisories (#60).
- The tracked workflow `.github/workflows/aien-verification.yml` builds only pinned verifier
  tools: the crumb tool from `aien-spark/aien-architecture` at
  `12ad0141b41ec67ef5aa51cb4e1815894c1083bb` and `aien-test` from `aien-spark/aien-sovereign-core`
  at `a51042b61b87355c0850ef82ba409a46e153a1ff`. On `main` the actions are referenced by tag
  (`@v4`, `@stable`); the separate workflow commit of this change pins each one by commit sha and
  adds `.github/workflows/sbom.yml`.
- AGENTS.md asks for `cargo audit`; no audit output is committed.

## Evidence

- `evidence/security-20261026-before.log`: security regression tests failing on the earlier
  `main` `f72209e6` (for example "environment crossed build boundary", "candidate test wrote a
  host-only file").
- `evidence/security-20261026-after.log`: the same suite after the fix, 33 passed, 0 failed.
- `evidence/security-20261026-s3.json` and `-fixture-compile.log`: workflow before, after and
  negative cases.
- `evidence/rsi-cut-2026-10-12/`: two real cycles against the fixture target, with raw files.
- `evidence/protocol-receipt-v1-superseded-2026-10-26/`: superseded receipt pins, kept unedited.
- Signed Spark bundles are published in aien-dev/aien-receipts, not here.
