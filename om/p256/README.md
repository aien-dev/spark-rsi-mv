# om/p256: the #67 judge signature scheme in seeded .om (library only)

`p256.om` is ECDSA over NIST P-256 with SHA-256 and RFC 6979 deterministic nonces. It reproduces the
signatures of the spark-rsi judge signer byte for byte. It is a parity-proven library and nothing more:

- it is **not a signer deployment**: nothing calls it from the judge, the daemon or the promotion gate;
- **no key was read**: every private key used here is a public test key (`[7;32]`, `[9;32]`, the RFC 6979
  A.2.5 key);
- **no receipt was signed**: the INTERPLANE receipts below were only verified, under their published key.

Refs spark-rsi-mv #67 (judge signer, ".om throughout") and #73 (REUSE-1, whose receipt is unsigned
because no `.om` signer existed).

## The scheme it matches

| Property | Rust judge | Source |
|---|---|---|
| Curve, crate | NIST P-256, RustCrypto `p256` 0.14.0 (`ecdsa` 0.17.0, `rfc6979` 0.6.0) | `Cargo.toml:41`, `Cargo.lock` |
| Key | 32-byte big-endian scalar, `SigningKey::from_slice` (0 and >= n refused) | `src/actor/judge.rs:1070-1082` |
| Message | the 32 raw bytes of `receipt_digest` (hex-decoded) | `src/evaluator/mod.rs:259-264` |
| Hash | `Signer::sign` hashes the message once with SHA-256; ECDSA signs that hash | `ecdsa-0.17.0/src/signing.rs:155-172` |
| Nonce | RFC 6979 section 3.2 HMAC-DRBG over (d, bits2octets(h)), no extra entropy (`ad = []`) | `ecdsa-0.17.0/src/signing.rs:171-172`, `rfc6979-0.6.0/src/lib.rs:73-100` |
| Output | `r \|\| s`, 64 bytes, big endian; the judge prefixes `tpm2-p256:` and hex | `src/evaluator/mod.rs:263` |
| Low-S | **not applied**: `p256` sets `NORMALIZE_S = false`, so s is whatever the equation gives and verification accepts both s and n - s | `p256-0.14.0/src/ecdsa.rs:60`, `ecdsa-0.17.0/src/hazmat.rs:85,142` |
| Public key | uncompressed SEC1, `0x04 \|\| X \|\| Y` (65 bytes) | `src/actor/judge.rs:1088` |

The low-S row is also checked by data: every parity vector carries a `V` line with s replaced by n - s,
and Rust accepted all 2048 of them (`parity-rust.txt`); `p2vrfy` gives the same verdict.

## Files

| File | What it is |
|---|---|
| `p256.om` | The library (682 lines): field and scalar arithmetic, points, HMAC-SHA-256, RFC 6979, sign, verify, public key. Functions and constants only. |
| `sha256.om` | Byte copy of aien-dev/omega-mv `om/sha256/sha256.om` at main `acb3f15c` (last change `a1cfb4a3`), sha256 `7f034814303a8a40b4bc56383ddd55ab2ae8460291f80793340bc3b0ac024e21`. The same bytes as `om/reuse-eval/sha256.om` on the REUSE-1 branch. Not modified. |
| `tests/run.om` | The runner: self tests, then every vector file named on the command line. Exit 0 only when every check passed. |
| `tests/rfc6979-a25.txt` | RFC 6979 A.2.5, P-256 with SHA-256: the public key, both nonces k and both signatures ("sample", "test"), copied from the RFC text. |
| `tests/parity-rust.txt` | Recorded from Rust `p256` 0.14.0: 2 public keys, 2048 signatures (1024 digests per key), 4864 verify verdicts. |
| `tests/edge-rust.txt` | Recorded from Rust: 3 signatures and 3 verdicts for a message whose SHA-256 is >= n. |
| `tests/interplane.txt` | 3 real judge receipts from aien-dev/interplane-mv `a68300c2`, each followed by a tampered copy, with Rust's verdicts. |

## API

Callers own four arrays and pass them in: `w ulong 1024` (workspace), `c uchar 104` (SHA-256 context),
`b uchar 4096` (byte buffer), `s ulong 64` (SHA-256 schedule). Call `p2init w` once.

| Call | Inputs | Result |
|---|---|---|
| `p2sign w c b s mlen` | key d at `b[3264..3296)`, message at `b[2048..2048+mlen)`, mlen <= 1024 | 0 and `r \|\| s` at `b[3200..3264)`; 1 when d is 0 or >= n |
| `p2vrfy w c b s mlen` | public key at `b[3072..3137)`, `r \|\| s` at `b[3200..3264)`, message as above | 1 valid, 0 refused |
| `p2pub w b` | key d at `b[3264..3296)` | 0 and `0x04 \|\| X \|\| Y` at `b[3072..3137)`; 1 when d is refused |

`p2vrfy` refuses a key that is not `0x04 || X || Y` with X, Y < p on the curve, r or s outside 1..n-1,
a sum u1 G + u2 Q equal to the identity, or x mod n different from r. These are the checks of
`VerifyingKey::from_sec1_bytes`, `Signature::from_slice` and `verify_prehashed` (`ecdsa-0.17.0/src/hazmat.rs:136-159`).

## How it is built

- 256-bit numbers are 8 limbs of 32 bits in ulong cells. omc has no 64x64 high product, and a 32-bit limb
  product plus two limbs is at most 2^64 - 1, so the Montgomery multiplication (CIOS, R = 2^256) never
  wraps a ulong where a carry is meant.
- One Montgomery multiplication serves both moduli (p and n). n0', R^2 and R are derived at `p2init`
  (Newton steps, 512 modular doublings), so the only hard-coded constants are p, n, b, Gx and Gy
  (SEC 2 v2 section 2.4.2). The self tests check G is on the curve, n G is the identity and
  (n - 1) G = -G.
- Points are projective with complete addition for a = -3 (Renes, Costello, Batina 2015, Algorithm 4,
  in the grouping of RustCrypto `primeorder` 0.14). The same formula doubles, so there are no special
  cases for the identity or equal inputs.
- Scalar multiplication is double-and-add-always over all 256 bits with a masked select.
- Inversion is Fermat (a^(m-2)) with the public exponent m - 2.

## Side-channel limits

This is stated for a future signer deployment; nothing in this PR deploys one.

- **Branch free with respect to secrets:**
  - field and scalar add, sub, mul and the final conditional subtraction (masked select, no branch);
  - the loop over the 256 bits of the secret scalar in `p2smul` (fixed trip count, masked select of
    the sum, array indices from public counters only);
  - inversion (the exponent m - 2 is public).
- **Not constant time:**
  1. The RFC 6979 retry branches on secret-derived values: k >= n, r = 0, s = 0. The Rust crate
     does the same, and these happen with probability about 2^-32 or less.
  2. `p2sign` returns early for an invalid key.
  3. omc gives no constant-time guarantee: its instruction selection and the bounds-check branches
     it emits are not audited for this. No timing measurement was made against secret inputs.
  4. The key, the nonce and the HMAC-DRBG state stay in the caller's arrays. They are never zeroed:
     the language has no zeroize, and the stack is not cleared.
- **Not variable-time optimized:** verification uses the same ladder twice, about 2x slower than a
  variable-time multi-scalar method. Its inputs are public, so this costs speed, not safety.

## Results (DGX Spark, Cortex-X925/A725, `nice -n 19`, one run)

```
self tests: 3 pass, 0 fail
file om/p256/tests/rfc6979-a25.txt: K 1/1 N 2/2 S 2/2 V 0/0 malformed 0
file om/p256/tests/parity-rust.txt: K 2/2 N 0/0 S 2048/2048 V 4864/4864 malformed 0
file om/p256/tests/edge-rust.txt: K 0/0 N 0/0 S 3/3 V 3/3 malformed 0
file om/p256/tests/interplane.txt: K 0/0 N 0/0 S 0/0 V 6/6 malformed 0
timing p2sign: calls 2053 mean_us 1230 min_us 1224 max_us 2694
timing p2vrfy (S lines, valid): calls 2053 mean_us 2337 min_us 2327 max_us 5086
timing p2vrfy (V lines, mixed): calls 4873 mean_us 2151 min_us 0 max_us 2433
p256: PASS
wall_s 20.22 user_s 20.22 sys_s 0.00 maxrss_kb 2080
```

What the lines mean:

- `S` counts a byte-equal signature **and** a verification of it under the public key derived by `p2pub`.
- `V` lines include s replaced by n - s, a tampered digest, the other key, r or s with one bit flipped,
  s = 0, r = n, and a key with Y changed.
- `p2vrfy` refuses some inputs early (range and format checks, timed 0 us). The mixed V-line timing
  therefore includes those refusals.
- The first call of a process is about 2x slower (cold caches): 2.7 ms sign, 5.1 ms verify on the RFC
  file alone.

## Mutants (throwaway copies under /tmp, not committed)

Every mutant was run over the four vector files. A mutant counts as detected only when the runner printed
at least one `MISMATCH` line and exited nonzero.

| Mutant | Exit | MISMATCH lines | Detected |
|---|---|---|---|
| M1: HMAC-DRBG V initialised to 0x02 instead of 0x01 | 1 | 2055 | yes |
| M2: one limb of Gy changed by 1 | 1 | 4110 (self tests 0/3) | yes |
| M3: final conditional subtraction removed | 104 (trap 4 on the first `N` line) | 1 (the RFC `K` line) | yes, then trapped |
| M4: low-S normalisation added to signing | 1 | 1059 | yes |
| M5: verification compares x with itself | 1 | 2435 | yes |
| M6: Montgomery reduction step removed | 1 | 4110 | yes |
| M7: h mod n removed before RFC 6979 and z | 1 | 3, all in `edge-rust.txt` (0 in the other files) | yes, only by `edge-rust.txt` |
| M8: on-curve check of the public key removed | 0 | 0 | **no** (see below) |
| M9: HMAC opad 0x5d instead of 0x5c | 1 | 2055 | yes |

**M8 survives.** Every off-curve key in the vectors is still refused without the check, because the
verification equation fails anyway.

- No vector here is an off-curve key that would verify. Building one against complete a = -3
  formulas, which use b, is an open problem; it was not attempted.
- The check matches `VerifyingKey::from_sec1_bytes` and stays.
- Wycheproof `ecdsa_secp256r1_sha256` and NIST CAVP SigVer are NOT_RUN.

## Reproduce

```sh
OMC=/path/to/omc   # the pinned seeded omc, sha256 734f0a39319fbfa930bcc62cb30dbf06d47d78e126372b74f5baf478e6126b61
cat om/p256/sha256.om om/p256/p256.om om/p256/tests/run.om | "$OMC" > /tmp/p256-run.elf
chmod +x /tmp/p256-run.elf
/tmp/p256-run.elf om/p256/tests/rfc6979-a25.txt om/p256/tests/parity-rust.txt \
  om/p256/tests/edge-rust.txt om/p256/tests/interplane.txt
```

Run it from the repo root. Exit 0 means every check passed; 1 means a mismatch or a malformed line; 2 means
a file could not be read.

## How the Rust data was recorded

The Rust data came from a throwaway program under `/tmp`, never committed. It uses `p256` 0.14.0 with
spark-rsi-mv's `Cargo.lock` and signs with `SigningKey::from_slice(key).sign(&msg)`, the same call as
`src/evaluator/mod.rs:262`. It verifies with `VerifyingKey::from_sec1_bytes` + `Signature::from_slice` +
`verify`.

- **Parity digests:** `msg_i = SHA-256(seed || u32be(i))` for i in 0..1024, where
  `seed = SHA-256("spark-rsi-mv om/p256 parity v1")`, signed under both keys.
- **Edge message:** found by brute force over `"spark-rsi-mv p256 h>=n  " || u64be(i)` (i = 2825506747).
- **INTERPLANE receipts:** the key is `provenance/fixtures/rsi-eval-v2/judge.pub` at interplane-mv
  `a68300c2`, and all three receipts verify under it in both Rust and `.om`.

## What remains (not in this PR)

1. **Key custody.** The real signing key lives with the judge account (`docs/RECEIPT-V2.md`, "Key
   separation"; `/var/lib/aien-judge`), and policy says atlas-vault. A `.om` signer has to load it there,
   under that account, without copying it. Nothing here touches it.
2. **RECEIPT-V2 digest in `.om`.** `compute_digest_v2` (`src/evaluator/mod.rs:143-201`) is still Rust
   only. A `.om` successor needs it byte-equal before it can sign what the Rust judge signs.
3. **REUSE-1 receipt mapping.** The REUSE-1 FINAL receipt (`om/reuse-eval/receipt.om`, identity domain
   `spark-rsi.reuse1-receipt.v2-unsigned`) is not a RECEIPT-V2 receipt. Signing it needs either a
   RECEIPT-V2-shaped record or a versioned companion domain (#67 item 6), plus the operator action in
   item 1.
   - Any such signature is written after FINAL and must be labelled so.
4. **Cutover.** The Rust signer is removed only after a `.om` signer is qualified and the judge and the
   promotion gate call it.
5. **Further vectors.** Wycheproof and NIST CAVP vectors, and an invalid-curve vector that kills M8.
