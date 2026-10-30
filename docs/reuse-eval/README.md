# REUSE-1 evaluator: judge

`om/reuse-eval/judge.om` is the REUSE-1 judge for spark-rsi-mv #73, written in seeded `.om`. It works from the frozen contract (omega-mv `d6edaad1`, `docs/reuse/REUSE-1.md`, sha256 `71a6da13...5deb`), the evaluator formats, the candidate `drive.om` CLI and built binaries. It reads no candidate source.

## Units and binary

Units are concatenated in this order, with no imports:

| Unit | File | Owner |
|---|---|---|
| sha256 | `om/reuse-eval/sha256.om` | byte copy of omega-mv main `acb3f15c` `om/sha256/sha256.om`, pinned in `sha256.pin` |
| ref | `om/reuse-eval/ref.om` | EvalGen |
| stats, turing, receipt | `om/reuse-eval/{stats,turing,receipt}.om` | EvalStats |
| judge | `om/reuse-eval/judge.om` | EvalJudge (prefix jd, JD, JW, JM) |
| main | `om/reuse-eval/main.om` | EvalJudge |

The single evaluator binary is `sha256 ref stats turing receipt judge main`. `main` sends `stats`, `turing` and `receipt` to the EvalStats entries and every other subcommand to `jdmain`.

## Commands

Paths are relative to the current directory unless stated. Run every measured command as `nice -n 10`. The judge puts each arm on a CPU with `/usr/bin/taskset -c CPU`; the contract places the evaluator on CPU 18 and the arms on CPU 19. A CPU of `-` means no taskset (tests only).

```
judge run SPLIT OUT REPS SEED ARMS BASE ALLOW LIB MAN LIBP MANP CPU
judge tap SPLIT BASE TAPFILE SPLITARG CPU
judge calib CALIBSPLIT RUNDIR REPS OUTFILE
judge build DRIVE OMEGADIR OMC ALLOW CALIB OUTLIB ACQDIR LIBID REP [DRIVE OPTIONS AND EXP FILES...]
judge recheck MANSHAFILE PINHEX
judge controls DEV BASE DRIVE OMEGADIR OMC ALLOW NOG3 C4 OUT CPU
```

- `run`: `ARMS` is a digit string of arm ids, for example `01234567`. `SEED` is the 64-hex FINAL seed (the arm-order seed is bytes 16..23, little endian) or a decimal seed for public splits. `LIB` and `MAN` are the TRAIN library and its MANSHA, used by method, delta and select. `LIBP` and `MANP` are the TRAINP library and its MANSHA, used by deltap. An unused pair is `- -`. Before each repetition R, the judge copies each library to `OUT/lib-R-1` and `OUT/lib-R-2`, so every repetition starts from a fresh copy. Arm configurations:
  - echo, full, cache: `BASE ARM - - - - -`
  - method, delta, select: `BASE ARM OUT/lib-R-1 MAN ALLOW - -`. Select reads the calibration that `drive build` baked into the library (integrator ruling).
  - idelta: `BASE delta - - ALLOW - -`
  - deltap: `BASE delta OUT/lib-R-2 MANP ALLOW - -`

  In each group and repetition, the base arm list is permuted with splitmix64 by Fisher-Yates (i = n-1 down to 1, j = next mod (i+1)). The seed state carries over across groups and repetitions in run order. Each arm process runs one group. The judge sends each request, waits up to 10 s for it, reads the whole response and times the round trip with `monons`. After the last request it sends STATS, reads `/proc/PID/stat` and `/proc/PID/status`, sends END and reaps the arm. A short read or end of file, a bad q (q above 32 or not the expected q), a timeout or a failed write is a protocol violation. The judge logs that request as wrong, kills the arm and records every remaining request of the group as not sent. It then goes on to the next group. `stdout` gets one `arm=A wrong=W violations=V` line per arm. `run` refuses to start if `OUT/proc.log` already exists.
- `tap`: runs one B process over every group of the split, in order, as `BASE tap - - - TAPFILE SPLITARG` (clarification K9). `TAPFILE` must not exist. `SPLITARG` is `1` for TRAIN and TRAINP, `2` for CALIB and `3` for DEV. `tap` prints `tap_sha256=`, `tap_bytes=`, `queries=`, `wrong=` and `violations=`. It exits 0 only when there is no wrong answer, the arm exits 0 and the file is not empty. Experience files are admitted by adding `<sha256> AUTHORIZED <label>` to ALLOW, which the operator does.
- `calib`: reads the CALIB split and the raw logs in `RUNDIR` of arms full (1), method (3) and delta (4), repetitions 0..REPS-1, and writes `OUTFILE` (R1CAL001, 200 words; `OUTFILE` must not exist). Sessions carry the family and n class. The change class of a QUERY is the number of accepted changes since the previous QUERY or BEGIN: SET, INS, DEL, SWAP and PROGRAM count 1, and BATCH counts m. Only QUERYs scored correct (word 15 = 1) enter the cells. Bind cells use method BEGIN round trips, excluding each process's first request (start-to-ready), minus the p50 of the full BEGIN round trips in the same family and n class, with a floor of 1. E_q x 1000 = floor(1000 x QUERY count / session count). Percentiles are nearest rank: index ceil(p n / 100) - 1 of the ascending values. The same rule is used in stats.
- `build`: runs `DRIVE build OMC ALLOW CALIB OUTLIB [options] EXP...` as a child process with cwd = `OMEGADIR`, and the judge's stdout captures the child's output. Elapsed time is `monons` around fork and reap. cutime and cstime are the deltas of the judge's own `/proc/self/stat` fields, which include drive and its omc children. The child is killed after 1 hour. `ACQDIR/acq.log` gets one 8-word record appended. The `manifest_sha256=` value is written to `ACQDIR/acq-LIBID-REP.mansha`. The exit status is drive's own (`drive_exit=` is printed).
- `recheck`: contract 7.4. It compares the first line of `MANSHAFILE` with `PINHEX`. A match prints `REBUILD MATCH` and exits 0. A mismatch prints `BLOCKED rebuild ... pinned ...` and exits 3. A mismatch is BLOCKED.
- `controls`: contract section 9 with clarifications K7, K10, K13 and K15. `OUT` holds private copies only, and the original ALLOW and libraries are never changed. See below.

Exit codes: 0 ok, 2 usage, 3 judge error (the `judge error N` line on stderr names the step), 4 `tap` failure or a STOP control in `controls`.

## Files written (EVAL-FORMATS)

All values are u64 little-endian words. R is 0-based, NN is the group number in 2 digits and A is the arm id.

- `OUT/arm-A-rep-R-gNN.log`: 16 words per request:
  - 0: group, 1: repetition, 2: arm, 3: request index, 4: op
  - 5: status got (2^64-1 marks the request that broke the protocol; 2^64-2 marks a request not sent after a kill)
  - 6: r0 got, 7: r1 got, 8: route got (QUERY only, otherwise 0)
  - 9..11: expected status, r0, r1
  - 12: round-trip ns, 13: cold flag, 14: session ordinal
  - 15: correctness: 1 ok; 0 wrong; 2 not scored, which means echo, or a non-QUERY whose status matched
- `OUT/proc.log`: 16 words per arm process, in execution order:
  - 0: arm, 1: group, 2: repetition
  - 3: start-to-ready, from `monons` just before fork to the end of the first response; this includes request 0's round trip
  - 4..7: utime, stime, cutime, cstime (ticks)
  - 8: VmHWM, 9: VmRSS (kB)
  - 10: exit word: the exit code, 256 + signal when killed by a signal, 512 + violation code when the judge killed the arm (1 short or end of file, 2 bad q, 3 timeout, 4 write; 5..8 the same four failures at END), 1023 when reaping failed. K17: 1024 is added when END itself failed (a response other than status 0, q 0, or any of codes 5..8). A nonzero exit after a good END keeps the plain exit code. Any nonzero word 10 is a failed process.
  - 11: requests sent, 12: sum of round-trip ns (END excluded), 13: CPU, 14: loadavg x 100, 15: END round-trip ns (K17; until failure when END failed)
- `OUT/stats-A-R-gNN.ctr` (K14): 34 words: STATS status, q, then q counters zero-padded to 32. Diagnostic only and not timed. The status word is 2^64-1 if STATS failed.
- `ACQDIR/acq.log`: 8 words per build:
  - 0: library id (1 TRAIN, 2 TRAINP, 3 EMPTY, 9 control), 1: repetition
  - 2: elapsed ns, 3: cutime, 4: cstime
  - 5: drive exit word, 6: LIBDIR regular-file bytes, 7: 0

  `ACQDIR/acq-L-R.mansha` holds 64 hex characters and a newline.
- `OUT/controls.log` (K7): 8 words per control: id, sub-case, outcome (1 detected, 0 undetected, 2 reported, 3 not run), served-tampered flag, 0, 0, 0, 0.

## Controls

The control workloads are derived from DEV. The ancestor is the first AGG BEGIN of DEV, and its first constant feeds the control program. The judge builds its own TRAIN-like experience with the tap arm of BASE: an ADD shape at wl 8 and an ADD shape at wl 16, each with 8 QUERYs. That gives a pair of shapes that differ only in wl, and wl-16 lane values of 256 and above. It authorizes that experience in a private copy of ALLOW and builds an honest wl-16 library, which serves as the reference for C3 and C6..C11. Every control workload is scored against `ref.om` (`rfreq`), never against candidate output. The ids are those of K7.

| K7 id | Contract | Sub-cases | Detected when |
|---|---|---|---|
| 1 | C1 | 0 `mutant:c1` with a wl-8 library, served wl 16; 1 the same plus `unit:check=NOG3` | 0: route 5 on every QUERY (G3) or wrong answers (K10; the printed `5=` and `wrong=` counts show which); 1: wrong answers |
| 2 | C1b | 0 `mutant:c1b`; 1 plus NOG3 | 0: route 5, no wrong answer; 1: wrong answers |
| 3 | C1c | 0 `mutant:c1c` | route 5, no wrong answer |
| 4 | C2 | 0 one byte of the magic; 1 one byte of the program digest (both copies re-authorized) | drive exit 10 |
| 5 | C3 | 0 sample outputs swapped; 1 diagnostic costs (word 7) rotated across records | 0: drive exit 10; 1: gen-G.om and gen-G.elf byte-identical, manifest word 4 and words 56..183 equal (K13) |
| 6 | C4 | 0 `unit:delta=C4` | wrong answers |
| 7 | C5 | per wl 8 and 16: full; delta with the library; idelta. Lanes 2^wl - 1 with MUL and SHL, values 2^wl | reference values, status 3, no wrong answer |
| 8 | C6 | 0 one ELF byte; 1 one manifest byte; 2 coherent replacement (new ELF, manifest and CURRENT consistent, MANSHA still the honest one) | every QUERY route 5 |
| 9 | C7 | 0 REVOKED appended before start; 1 REVOKED appended before request 2 | 0: route 5 throughout; 1: method or delta routes before, route 5 on every later QUERY |
| 10 | C8 | K = 1..9 `crashafter:K` over a copy of the honest generation P (K15) | exit 97; CURRENT, gen-G.man and gen-G.elf are self-consistent and load with their own MANSHA; no wrong answer, and method or delta routes served |
| 11 | C9 | 0 compiler SHA-256; 1 profile version (manifest, self-check, CURRENT and MANSHA rewritten coherently) | route 5 |
| 12 | C10 | 0 manifest truncated to 777 bytes; 1 CURRENT truncated to 17 bytes | route 5 |
| 13 | C11 | SIGKILL after request 3, then a fresh arm | the kill is observed (exit word 265); CURRENT and MANSHA are unchanged; the fresh arm answers with no wrong answer |
| 14 | C12 | reported (outcome 2); scored by `run` with arms 5 and 6 | ablation |

A control that cannot be set up (a failed build, a missing CURRENT) is recorded as 3 (not run), never as passed. `controls` exits 4 if any outcome is 0 or any served-tampered flag is set (STOP), exits 3 if a control was not run, and exits 0 otherwise.

## Holdout: running FINAL once (contract 7.3)

1. Evaluator freeze: commit and push this tree, together with `evidence/REUSE-73/final-commitment.txt` (SHA-256 of the secret).
2. Candidate freeze: omega-mv commit with the TRAIN and CALIB library and its MANSHA.
3. `judge build` rebuilds the TRAIN library from the admitted experience with the frozen drive, three times (REP 0..2), and `judge recheck` checks each result against the frozen MANSHA. A mismatch is BLOCKED, and the run stops there.
4. The operator reveals the secret. FINAL seed = SHA-256(secret || candidate commit || evaluator commit).
5. `gen FINAL OUTDIR SEEDHEX`, then once: `nice -n 10 taskset -c 18 eval judge run FINALDIR RUNDIR 3 SEEDHEX 01234567 BASE ALLOW LIB MAN LIBP MANP 19`.
6. Run `stats`, `turing` and `receipt` on that run directory. A void run burns the seed, and any later run needs a new secret and a recorded reason.

## Tests

From the repo root: `OMC < tests/reuse-eval/run.om > run.elf`, then `run.elf OMC [PREFIX]`.

- The first line of a test is `note units: ...`. An optional second line `note needs: gen|eval|arm|drive|omc ...` builds those binaries once and passes their paths as argv[2..]; argv[1] is a fresh scratch directory.
- A line `note fixture:` marks a helper program, which is compiled for `needs` and never run as a test.
- Each test may run for at most 120 s. The runner prints `PASS` or `FAIL name rc=N`, with 2048 + rc for a compile failure, and then `TOTAL`.

`judge-arm.om` is a fake arm built on `ref.om`. Its mode comes from the ARM word: full; wrong (r0 + 1); echo; badq; short; stall (sleeps 10.1 s); tap (writes R1EXP001 records); endbad (END answered q 1), endst (END status 3), endeof (no END answer), endexit (good END, exit 7). `judge-drive.om` is a fake drive. The fakes check only the judge's protocol, logs and fail-closed behaviour. They are not the candidate. `judge-end.om` also pins the arm order: for splitmix64 seed 0 over arms 01234567, the first two permutations are 2,5,0,3,4,6,1,7 and 3,7,0,5,6,2,1,4 (computed outside the judge from the standard splitmix64 constants).
