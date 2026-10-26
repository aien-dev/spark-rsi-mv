# Security policy

## Reporting a vulnerability

Report privately. Do not open a public issue, pull request or discussion about it.

1. GitHub: open this repository's Security tab and choose "Report a vulnerability". This
   creates a private advisory that only the maintainers can see. If that option is not shown,
   private vulnerability reporting has not been enabled for this repository yet; use email.
2. Email: drake@aienos.com with the subject `[spark-rsi-mv] security report`.

Include the commit or release you tested, the component (path), steps to reproduce, and what an
attacker gains. If the report involves a secret, say where it was exposed and do not send the
secret itself. Encrypted mail is not required; do not post proof-of-concept payloads publicly.

## Response targets

This project has one maintainer; the targets below are best effort and stated honestly.

- Acknowledgement: within 3 working days.
- Triage and severity: within 7 days of acknowledgement.
- Fix or mitigation for confirmed issues: aim for 30 days for high and critical, 90 days
  otherwise, with a status update at least every 14 days until closed.
- Disclosure: coordinated. An advisory is published with credit (unless you ask otherwise)
  once the fix is on the default branch. Default embargo 90 days, negotiable.

## Scope

Spark RSI is a native Rust engine that observes a target repository, asks a model for small
improvement proposals, tests each one in a sandbox copy, and hands a human a review branch. It
never commits to the target's default branch. It builds three programs: `spark-rsi`,
`spark-rsi-supervisor` and `spark-rsi-judge`.

In scope:

- Escape from the build jail or the canary jail. The containment rules this repository states:
  - Candidate Cargo checks, tests and observation build scripts execute only in bubblewrap with
    read-only source, separate private output, no network and a cleared environment. Offline and
    locked dependency resolution is mandatory. A transient systemd user cgroup enforces configured
    memory, process, CPU and build-runtime limits; there is no unbounded host fallback.
  - BuildJail clears the launcher and guest environments. Its writable output must be disjoint
    from source and belongs to one candidate; a shared host target cache is not an artifact
    authority.
  - Canary workers run in user, mount, PID, IPC, UTS and network namespaces. Only their
    executable, system runtime, private scratch and one IPC socket are exposed. Supervisor
    evidence and expected outputs stay outside the jail. No unjailed fallback is permitted.
- A candidate reaching the target's default branch, or the ratify step accepting a path outside
  the declared `ratify_roots` (`..`, symlinks, submodules, deletions are meant to be refused).
- The promotion gate accepting a candidate without a version 2 receipt from a separate judge, or
  accepting one evaluated by the in-process judge.
- Forged or replayed receipts, ledger signatures or plan digests.
- Plaintext secrets written to disk by the tools.

Out of scope:

- The model server, the Cortex memory service, the `aien-protocols` sibling crates (report those
  in their own repository), bubblewrap, systemd and the Linux kernel.

The containment depends on Rust/Cargo, Linux bubblewrap and a systemd user manager with cgroup v2
resource controllers. Exercised versions recorded by the previous policy: Rust/Cargo 1.98.1,
bubblewrap 0.9.0, systemd 255 (255.4-1ubuntu8.17). These are host runtime dependencies, not Omega
implementations.

## Supported versions

Only the default branch `main` (Cargo version `0.1.0`). README marks the project pre-alpha. Reports against older commits are welcome but fixes land on the default branch only.

## Status

Pre-alpha. README: development and evaluation continue with no unattended self-modification and
no automatic promotion. No certification, no bug bounty. See AUDIT_SCOPE.md for what to review and how to build and test it.
