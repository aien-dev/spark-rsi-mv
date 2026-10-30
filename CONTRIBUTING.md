# Contributing to spark-rsi

We welcome contributions from sovereign systems engineers and developers passionate about autonomous agent infrastructure.

Development follows [Human-Directed Agentic Engineering (HDAE)](README.md#human-directed-agentic-engineering).
Humans set goals, architecture, constraints, acceptance criteria, and stopping points.
Agents implement, execute checks, diagnose failures, and revise within that scope;
human review retains responsibility for claims, acceptance, and release decisions.
The README cites Gregory M. Kurtzer's 2026 Ginux report as a documented method and
systems-software case study, not as a claim about the protocol used by past experiments.

## Sovereign Principles & Invariants

All contributions must adhere to our engineering invariants:
1. Unslop Standard: Zero em dashes or en dashes. Use standard punctuation (commas, colons, parentheses, periods). Forbid conversational filler and generative cliches.
2. Native Performance: Pure compiled native Rust architecture. Eliminate runtime interpreter overhead.
3. Hardware Key Vault: Never commit plaintext .env files, credentials, or private keys.
4. Upstream Contribution: Public components modified to work on our stack must be contributed back upstream.

## Verification
```bash
cargo test --verbose
```
