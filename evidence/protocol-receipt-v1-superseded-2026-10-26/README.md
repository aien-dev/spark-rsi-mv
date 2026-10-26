# SUPERSEDED: protocol_version 1 signed evaluation receipt pins (frozen 2026-10-12)

Status: **superseded on 2026-10-26**, retained as historical evidence. Do not edit the files in this
directory, and do not move them back into `tests/`: the protocol no longer accepts them.

## What is retained

`test_canary_contract_golden.aef09a6.rs.txt` is `tests/test_canary_contract_golden.rs` exactly as it
stood at spark-rsi `aef09a6` (main after #77), copied with `git show`. It is stored with a `.txt`
suffix outside `tests/` so that cargo never compiles or runs it.

| Check | Value |
|---|---|
| git blob of `aef09a6:tests/test_canary_contract_golden.rs` | `2f35e477804a1accfc82c94b30509f2d273ded6a` |
| `git hash-object` of the retained file | `2f35e477804a1accfc82c94b30509f2d273ded6a` |
| SHA-256 of the retained file | `0da6d3c582108f93113d4645c37fc312ca55bd45d66c255ac5e32ef94cc4ae9e` |

The superseded part is family (2)'s protocol receipt test,
`protocol_signed_evaluation_receipt_digest_and_signature_are_frozen`, and its pins:

| Pin | Value |
|---|---|
| `EXPECTED_PROTOCOL_KEY_FINGERPRINT` (still current) | `8cbc48b78519c7a8a48f92d6e9e60f06645e872e6d148b660a5aef6afc990905` |
| `EXPECTED_PROTOCOL_RECEIPT_DIGEST` (protocol_version 1, unframed) | `a7f3ac2e08162a5f2f49805d8cf4f6c8a7f38fe6bb0f2e74437d9bbfce1cdccf` |
| `EXPECTED_PROTOCOL_RECEIPT_SIGNATURE` | `41b9f0735b4ecceb3e390eeb1eca82f09173ce393ab0bbb8f03d4b5c55a216027d1d0df88d5ac064ac70d7ecd63d29af8e3de04c3ff09dada42a6fa8e09c7240` |

Every other pin in the retained file (canary workload, observation, sealed log, spark-rsi receipt
v1 and v2, ledger) is unchanged and still enforced by the active test.

## Why it was superseded

[aien-protocols-mv#59](https://github.com/aien-dev/aien-protocols-mv/pull/59) (SECNET-02, approved by
Drake as a breaking change) replaced the unframed digest concatenations, in which adjacent strings
could trade bytes without changing the hash, with domain-separated, length-framed transcripts:
`AIEN_EVALUATION_PLAN_V2`, `AIEN_EVALUATOR_OUTCOME_V2` and `AIEN_EVALUATION_RECEIPT_V3`. The only
receipt `protocol_version` the protocol signs or verifies is now 3, and there is no fallback to the
old digest. The pins above therefore cannot be reproduced or verified by the current protocol, and
their replacement is a new version with new pins, not an edit of these.

The replacement pins live in the active `tests/test_canary_contract_golden.rs`
(`EXPECTED_PROTOCOL_PLAN_V2_DIGEST`, `EXPECTED_PROTOCOL_RECEIPT_V3_DIGEST`,
`EXPECTED_PROTOCOL_RECEIPT_V3_SIGNATURE`), generated with that file's `print_pins` test against
aien-protocols candidate `28bc49ec57a89398fcb463cf71c0add7fcd2ef6c`. The receipt fixture is the
2026-10-12 one with only `protocol_version` changed from 1 to 3.

## Logs

- `pre-cutover-protocols-f7b3271.log`: spark-rsi `aef09a6` with `../aien-protocols` at `f7b3271`
  (protocols main before #59). The retained tests, including the superseded protocol receipt test,
  pass: 10 passed, 1 ignored, exit 0. This is the last layout in which these pins hold.
- `unchanged-fixture-on-candidate-28bc49e.log`: the same spark-rsi `aef09a6` with
  `../aien-protocols` at `28bc49ec57a89398fcb463cf71c0add7fcd2ef6c`. The superseded test fails on its
  digest pin (computed `cd55d489…581a`, pinned `a7f3ac2e…dccf`), exit 101.

## Reproducing the historical result

```bash
git clone https://github.com/aien-dev/spark-rsi-mv spark-rsi
git clone https://github.com/aien-dev/aien-protocols-mv aien-protocols
git -C aien-protocols checkout f7b3271
cd spark-rsi && git checkout aef09a6
cargo test --locked --test test_canary_contract_golden
```
