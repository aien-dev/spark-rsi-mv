# REUSE-1 FINAL evaluation (spark-rsi-mv #73)

Decision: **STOP** (contract REUSE-1 v1.0 section 10: the lower bound of rho(delta) is at least 1).
Correctness: 0 wrong answers and 0 wrong refusals in 189,504 scored requests across 7 scored arms
and 3 repetitions. Every negative control was detected (36 of 36 required cases; C12 is reported only).

## Pins

| Item | Value |
|---|---|
| Contract | omega-mv `d6edaad1` `docs/reuse/REUSE-1.md`, sha256 `71a6da13...5deb`; clarifications K1..K17 |
| Evaluator freeze | spark-rsi-mv `4345eda9e58fe625a4d9b8e9175305160369f760` (pushed 2026-10-25T16:20:47Z), evaluator ELF sha256 `b550ad7e...46cf` |
| Candidate freeze | omega-mv `6ad09bff0c544bf73ceb3c63b349ba0cd6998e52` (pushed 2026-10-25T16:21:52Z), source `2b25e1bf`, base ELF `bf7dd018...2568`, drive ELF `ade1c049...9825` |
| Compiler | omc sha256 `734f0a39...6b61` |
| TRAIN library | MANSHA `05f163f8...7cf6`, 16 templates, calibration `c7971e77...6ab0` |
| TRAINP library | MANSHA `b15f71dc...4c61` |
| FINAL | seed commitment pushed 2026-10-25T15:10:36Z; seed revealed in `seed-reveal.txt`; holdout digest `1ce74aa2...8f04`; 24 groups, 192 sessions, 17,952 requests, 9,024 QUERYs per arm and repetition |
| Run | 2026-10-25, 16:22:12Z to 16:22:26Z UTC, judge on CPU 18 and arms on CPU 19 (X925), nice 10, load average 7.1 (shared host with live model services) |

## Results (95 % cluster bootstrap over 24 groups, 10,000 resamples)

| Arm | rho (point) | 95 % interval |
|---|---|---|
| full | 1.000 | 1.000 to 1.000 |
| echo (pipe-overhead control) | 0.521 | 0.459 to 0.598 |
| cache | 3.179 | 2.805 to 3.603 |
| method | 1.219 | 1.091 to 1.363 |
| delta (primary) | 1.164 | 1.040 to 1.307 |
| idelta (delta, empty library) | 0.751 | 0.650 to 0.867 |
| delta' (shape-permuted library) | 1.011 | 0.895 to 1.143 |
| select | 1.001 | 0.887 to 1.127 |

Ratios: delta/cache 0.366 (0.304 to 0.440); delta/idelta 1.551 (1.406 to 1.737); select/delta
0.859 (0.759 to 0.957); delta/method 0.955 (0.854 to 1.079). Transfer coverage of delta: 100 %
(5,616 of 5,616 transfer QUERYs served by a method).

The full per-arm, per-category latency quantiles (cold and warm p50, p95 and p99), memory, Cbar
terms, payback and regime tables are in `stats.txt`.

Turing (`turing.txt`): B and both library models code D (9,024 symbols) to identical lengths
(60 bytes, rANS and range, H1 PASS), so T = minus the library charge. T(delta) = -8,192 bits
(P1), -80,952 bits (P2); T(select) = -20,544 bits (P1). No positive T exists in this profile.

Receipt (`receipt.txt`): UNSIGNED (no `.om` P-256 signer; the #67 signer is Rust).
Independence: UNISOLATED.

## Reproduce

From the two freeze commits, with the pinned omc:

```
OMC=<omc 734f0a39...>
# candidate (omega-mv 6ad09bff): build the test runner, drive and the baseline router
$OMC < tests/reuse/run.om > run.elf && run.elf compose $OMC om/reuse/drive.om drive.elf
drive.elf base $OMC base
# evaluator (spark-rsi-mv 4345eda9)
cat om/reuse-eval/{sha256,ref,stats,turing,receipt,judge,main}.om | $OMC > eval.elf
cat om/reuse-eval/{sha256,ref,gen}.om | $OMC > gen.elf
gen.elf FINAL splits/FINAL <final_seed from seed-reveal.txt>
eval.elf build drive.elf <omega checkout> $OMC allow.txt calib.r1cal lib-T acq 1 0 exp-TRAIN.bin
eval.elf run splits/FINAL finalrun 3 <final_seed> 01234567 base/base.elf allow.txt lib-T <MANSHA T> lib-P <MANSHA P> 19
eval.elf controls splits/DEV base/base.elf drive.elf <omega checkout> $OMC ctl-allow.txt tests/reuse/mutants/nog3-check.om tests/reuse/mutants/c4-delta.om finalctl 19
eval.elf stats splits/FINAL finalrun finalctl <final_seed> finalstats
eval.elf turing splits/FINAL calibration/profiles/Turing-profile-REUSE-v1.0.toml lib-T lib-P base/base.om <turing-coder> finalturing
eval.elf receipt splits/FINAL finalrun finalstats docs/reuse/REUSE-1.md 6ad09bff... <MANSHA T> receipt.txt
```

Experience files, `allow.txt`, `calib.r1cal` and the library sources are in omega-mv
`evidence/REUSE-742/freeze/`. Raw evidence here: `finalrun.tar.xz` (raw logs, proc, STATS
counters, acquisition), `finalctl.tar.xz` (controls), `final-split.tar.xz` (the FINAL requests and
expectations), `calibrun.tar.xz`, `finalturing.tar.xz` (TPS1 and coded streams).
`SHA256SUMS` lists every file.

## Verdicts by evidence class

| Class | Verdict |
|---|---|
| Source | PASS after independent review: candidate and evaluator reviewed by separate read-only reviewers; 2 + 4 findings fixed and verified, plus 1 regression fixed |
| Host (Spark CPU) | Correctness PASS; controls PASS; lifecycle cost decision STOP |
| Simulator | NOT_RUN |
| Physical (GB10) | NOT_RUN |
| Energy | NOT_MEASURED |
