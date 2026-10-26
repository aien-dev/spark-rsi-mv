# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of sovereign agent infrastructure seriously. If you discover a vulnerability, please report it responsibly rather than opening a public issue.

### Reporting Channels
- Email: aien@aienos.com
- Key: Available upon request or via public keyservers.

### Sovereign Security Invariants
- Zero plaintext API tokens or keys on disk. All integrations must use hardware TPM vault or secure loopback credential injection.
- Loopback isolation: Daemons and internal IPC default strictly to 127.0.0.1.
- Safe deserialization: Untrusted inputs are strictly schema-validated before processing.
- Candidate Cargo checks, tests and observation build scripts execute only in bubblewrap with read-only source, separate private output, no network and a cleared environment. Offline and locked dependency resolution is mandatory. A transient systemd user cgroup enforces configured memory, process, CPU and build-runtime limits; there is no unbounded host fallback.
- BuildJail clears the launcher and guest environments. Its writable output must be disjoint from source and belongs to one candidate; a shared host target cache is not an artifact authority.
- Canary workers run in user, mount, PID, IPC, UTS and network namespaces. Only their executable, system runtime, private scratch and one IPC socket are exposed. Supervisor evidence and expected outputs stay outside the jail. No unjailed fallback is permitted.

The current containment implementation depends on Rust/Cargo, Linux bubblewrap and a systemd
user manager with cgroup v2 resource controllers. Exercised versions: Rust/Cargo 1.98.1,
bubblewrap 0.9.0, systemd 255 (255.4-1ubuntu8.17). These are explicit host-runtime dependencies,
not Omega implementations. An Omega replacement must provide the same namespace, mount,
environment, cgroup and descendant-lifetime boundaries before replacing them.
