# Spark RSI: Recursive Self-Improvement Engine

[![License: AGPL-3.0-or-later](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue.svg)](LICENSE)

Spark RSI is a native Rust engine that tries to improve a code repository on its own. It observes the target, asks a model for small improvement proposals, tests each one in a sandbox copy, and hands a human a review branch. It never commits to the target's default branch. The model is any OpenAI-style chat completions endpoint (the local model seat), given with `--model-url` and `--model-id`. Some defaults are specific to the DGX Spark: the model endpoint `http://127.0.0.1:18006/v1`, the model name `atlas-lightning-omni` and the Cortex memory service `http://127.0.0.1:18080` (defaults in `src/main.rs`).

## Where it fits in AIEN

Spark RSI is an attachment of [AienOS](https://github.com/aien-dev/aienix), the core tree of AIEN. It lives in its own repository and plugs into the core at a boundary. Related repositories:

- [aien-dev/aien-protocols-mv](https://github.com/aien-dev/aien-protocols-mv): the protocol crates this repository builds against (`aien-protocol-types`, `aien-evaluation-protocol`, `aien-probe`).
- [aien-dev/aien-architecture-mv](https://github.com/aien-dev/aien-architecture-mv): the AIEN plans and decisions, including the release ruling quoted below.
- [aien-dev/aien-sovereign-core-mv](https://github.com/aien-dev/aien-sovereign-core-mv): the Spark test pipeline (`tools/aien-test`, `tools/aien-local-ci`) that judges every pull request here.
- [aien-dev/aien-receipts](https://github.com/aien-dev/aien-receipts): where the signed test bundles for this repository are published.

## Status

**Status (2026-10-19): pre-alpha, not part of the WHOLE-SYSTEM-E2E v1 release.** This is Drake's ruling 4 of 2026-10-10 ([INTEGRATION_PLAN.md, section 6](https://github.com/aien-dev/aien-architecture-mv/blob/main/docs/plans/release-readiness/INTEGRATION_PLAN.md)): development and evaluation continue independently, with no unattended self-modification and no automatic promotion.

- The daemon's promotion gate requires a version 2 receipt from a separate judge ([docs/RECEIPT-V2.md](docs/RECEIPT-V2.md)) and refuses every candidate its own in-process judge evaluated.
- The last feature work merged on 2026-10-12 (pull requests #44, #48 and #51): supervisor-verified canary evidence, measured stage timings, version 2 receipts from the CLI, and ledger signing keys kept off the command line. The plan is [docs/plans/2026-10-12-evidence-driven-rsi-cut.md](docs/plans/2026-10-12-evidence-driven-rsi-cut.md).
- Changes since then: Modular MAX, Mojo and Python were removed (2026-10-15, #55 and #56). Testing moved to AIEN's Spark pipeline (2026-10-16, #58). The build jail now finds a Rust toolchain outside the home directory, and rustls moved to 0.23.46 for four security advisories (2026-10-18, #60). The hosted GitHub test workflows were deleted (2026-10-18, #62).
- Open tracking issues: [#54](https://github.com/aien-dev/spark-rsi-mv/issues/54) (qualify one independent version 2 evaluation and review-branch cycle, for release tracking issue arch#190) and [#57](https://github.com/aien-dev/spark-rsi-mv/issues/57) (use generalization evidence as a testable RSI hypothesis, without automatic promotion).

## Copyright

Copyright (c) 2026 Drake Stapleton <aien@aienos.com> and AIEN Contributors. Authored by Drake Stapleton in collaboration with AIEN.

---

## Build and test

Spark RSI is not a standalone checkout. It depends on the protocol crates by path (`Cargo.toml`), so [aien-protocols-mv](https://github.com/aien-dev/aien-protocols-mv) must be cloned next to it under the name `aien-protocols`:

```bash
git clone https://github.com/aien-dev/spark-rsi-mv spark-rsi
git clone https://github.com/aien-dev/aien-protocols-mv aien-protocols
cd spark-rsi
cargo build --locked
cargo test --locked -- --test-threads=1
```

This builds three programs: `spark-rsi` (the main command line), `spark-rsi-supervisor` (the host supervisor that runs candidate workers) and `spark-rsi-judge` (the separate judge that writes evaluation receipts). Run any of them with `--help` to see its options.

The build jail runs `bwrap` (bubblewrap), which must be installed on the machine that runs the tests. Every jail that runs cargo finds the Rust toolchain the way cargo and rustup do: `CARGO_HOME` and `RUSTUP_HOME` when set (else `$HOME/.cargo` and `$HOME/.rustup`), plus the `PATH` directory holding `cargo`. It binds those directories read-only inside the jail and sets the same variables there, so a host whose toolchain lives outside the home directory (the Spark judge keeps it in `/var/lib/aien-run/cargo` and `/var/lib/aien-run/rustup`) still builds candidates. A directory that is or holds the judge's home is never exposed (`src/isolation/cargo_sandbox.rs`). The balance kernel is native Rust (`src/balance.rs`); it needs no extra toolchain.

### Protocol digest cutover (aien-protocols#59)

Since 2026-10-26 this repository requires the strict framed digest contract of [aien-protocols-mv#59](https://github.com/aien-dev/aien-protocols-mv/pull/59): `EvaluationPlan` digests use the `AIEN_EVALUATION_PLAN_V2` transcript and signed `EvaluationReceipt`s are `protocol_version` 3 with the `AIEN_EVALUATION_RECEIPT_V3` transcript. The protocol signs and verifies no other receipt version and has no fallback to the old unframed digests, so a receipt or plan digest produced before the cutover does not verify and must be produced again by a new evaluation.

The `aien-protocols` sibling must contain that change. Until #59 is merged, check the sibling out at the reviewed candidate `28bc49ec57a89398fcb463cf71c0add7fcd2ef6c` (`git -C aien-protocols checkout 28bc49ec57a89398fcb463cf71c0add7fcd2ef6c`); after the merge, `main` carries it. Against a sibling from before #59, `tests/test_canary_contract_golden.rs` and `tests/safety_envelope_tests.rs` do not compile, because they name the version 3 constant and its refusal error. The protocol_version 1 receipt pins this repository froze on 2026-10-12 are kept unedited, and marked superseded, in [evidence/protocol-receipt-v1-superseded-2026-10-26/](evidence/protocol-receipt-v1-superseded-2026-10-26/).

### How pull requests are tested

The DGX Spark is the only test judge for this repository. Since 2026-10-16 it runs the gates in [tests/gates/](tests/gates/) on every pull request, and since 2026-10-18 there are no hosted GitHub workflows that build or test this code (#62). The one required check on `main` is `AIEN Verification` ([.github/workflows/aien-verification.yml](.github/workflows/aien-verification.yml)). It does not build the code; it checks the signed bundle the Spark published to [aien-dev/aien-receipts](https://github.com/aien-dev/aien-receipts/tree/main/spark-rsi).

The gates, all run through `tests/gates/run.sh <step>`:

| Gate | Step | What it runs |
|---|---|---|
| `CRUMB-VERIFY` | `crumb-verify` | `crumb verify .` (every commit) |
| `RSI-ZERO-DISK-SECRETS` | `zero-disk-secrets` | refuses any `.env` file (every commit) |
| `RSI-FMT` | `fmt` | `cargo fmt --all -- --check` |
| `RSI-CHECK` | `check` | `cargo check --locked --all-targets` |
| `RSI-TEST` | `test` | release build, then `cargo test --locked --verbose -- --test-threads=1` |

The first two are listed in [.aien/always](.aien/always) and run on every commit. The others run when their input files change. How a pull request gets merged is in [docs/PIPELINE.md](docs/PIPELINE.md).

### Version 2 judge receipts from the CLI

`spark-rsi judge` writes a version 2 receipt whenever an evaluation policy is configured, with `--policy-file` or `RSI_JUDGE_POLICY_FILE`. A policy requires `--subject-path` (the one file the receipt binds) and normally `--executable`. A malformed policy, or a policy without a subject, is refused rather than downgraded. Without a policy the judge writes a version 1 receipt, which is history only and never promotes. `spark-rsi cycle` and `spark-rsi daemon` take the promotion gate's pins as `--judge-policy-file` / `RSI_JUDGE_POLICY_FILE` and `--judge-public-key` / `RSI_JUDGE_PUBLIC_KEY`, and refuse malformed pins at startup.

---

## Operator profile

Create or update your local operator profile. With no flags, `init` saves the values already in the file, or the defaults when there is no file. It does not ask questions:

```bash
spark-rsi init
```

To set values, give them as flags (`--name`, `--email`, `--mode` as `local` (the local model seat) or `api`, `--api-url`, `--model-id`):

```bash
spark-rsi init \
  --name "Your Name" \
  --email "you@domain.org" \
  --mode "local" \
  --model-id "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16"
```

The profile is saved to `~/.config/sovereign/operator.toml` (set `SOVEREIGN_CONFIG_PATH` to use another file). The commit on the review branch and the ratification record use the name and email you set here.

The saved engine settings (`mode`, `api_base_url`, `model_id`) are shown by `spark-rsi profile` but are not read by `cycle`, `daemon` or `soak`. Those commands take the model endpoint from `--model-url` and `--model-id`, and send no API key.

---

## The 5-phase cycle

`spark-rsi cycle` runs one cycle; `spark-rsi daemon` runs them in a loop.

1. **Observe**: reads the git branch and uncommitted files, the number of `.crumb` and `.crumb.local` files, whether `cargo check` passes, and a drive/humanity term count (soul tension) from `soul.md`, `docs/PHILOSOPHY.md` and `README.md` (`src/observe.rs`).
2. **Propose**: asks the model for small improvement proposals and applies each in an isolated sandbox copy of the target tree (`/tmp/spark-rsi-sandbox/` for `cycle` and `daemon`).
3. **Verify**: checks the unslop writing standard, refuses plaintext secrets on disk, and runs `cargo check` and `cargo test`.
4. **Balance**: scores drive against humanity with the native Rust balance kernel (`src/balance.rs`, CPU only).
5. **Ratify**: never commits to the target's default branch. It builds the candidate commit off to the side and runs the protected-file checks on the real git diff (path normalization; `..`, symlinks, submodules, deletions and anything outside the declared `ratify_roots` refused). Then it creates a review branch `rsi/<proposal id>` and a patch ready for a pull request under `<rsi_root>/patches/`. A human opens and merges the pull request.

---

## Evidence and documents

- [evidence/rsi-cut-2026-10-12/REPORT.md](evidence/rsi-cut-2026-10-12/REPORT.md): two real cycles on the DGX Spark against the fixture target on 2026-10-12, one correct and one with a wrong answer, with every raw file. It qualifies the canary mechanism only, not self-improvement.
- [evidence/dual/receipt.md](evidence/dual/receipt.md): the DUAL diagnostic price reader (diagnostic only, no authority).
- [fixtures/canary-target/](fixtures/canary-target/README.md): the small worker used as the first canary target. `examples/canary_qualification.rs` runs one cycle against it.
- [docs/CANARY-CONTRACT.md](docs/CANARY-CONTRACT.md): how the supervisor drives and checks canary requests.
- [docs/RECEIPT-V2.md](docs/RECEIPT-V2.md): the version 2 evaluation receipt.
- [CONTEXT.md](CONTEXT.md): the glossary of terms used in the code and docs.
- [docs/PHILOSOPHY.md](docs/PHILOSOPHY.md): the programming philosophy.

---

## License and governance

Licensed under the GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later). See [LICENSE](LICENSE). Project values live in the nonbinding [COVENANT.md](COVENANT.md), which grants and restricts no legal rights.

All downstream distributions, derivative works and commercial deployments are governed only by the terms of [LICENSE](LICENSE). [CONSTITUTION.md](CONSTITUTION.md) is the internal architecture charter and development doctrine for upstream engineering.

## Human-Directed Agentic Engineering

Development follows Human-Directed Agentic Engineering (HDAE): humans define goals,
architecture, constraints, acceptance criteria, and stopping points; agents implement,
execute checks, diagnose failures, and revise within those boundaries. Human review
retains responsibility for claims, acceptance, and release decisions. This describes
how the project is developed, not a change to the engine's runtime proposal loop.

For the documented method and a systems-software case study, see Gregory M. Kurtzer,
[*Ginux: Building an Independent Kernel That Runs Linux Userspace through
Human-Directed Agentic Engineering* (2026)](https://doi.org/10.5281/zenodo.23682548).
