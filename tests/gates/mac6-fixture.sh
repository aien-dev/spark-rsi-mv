#!/bin/bash
# mac6-fixture.sh: the Allen MAC-6/7 development fixtures are records, not a run.
# Checks the pins and the safety flags the manifest claims. Host-only. No model,
# no network, no GPU. A wrong pin or a promotion flag that is not false fails.
set -eu
export LC_ALL=C
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fix="${MAC6_FIX:-$root/experiments/allen-mac6}"
python3 - "$fix" << 'PY'
import hashlib, json, sys
from pathlib import Path
fix = Path(sys.argv[1])
man = json.loads((fix / "manifest.json").read_text())
def sha(name):
    return hashlib.sha256((fix / name).read_bytes()).hexdigest()
pins = man["pins"]
checks = [
    ("development_tasks.json", pins["development_tasks_sha256"]),
    ("negative_controls.json", pins["negative_controls_sha256"]),
]
for name, want in checks:
    got = sha(name)
    if got != want:
        sys.exit(f"MAC6_PIN {name} got {got} want {want}")
tasks = json.loads((fix / "development_tasks.json").read_text())
cases = tasks["cases"]
if man["task_suite"]["case_count"] != 30 or len(cases) != 30:
    sys.exit(f"MAC6_COUNT claimed {man['task_suite']['case_count']} listed {len(cases)}")
if man["status"] != "fixture_only_not_executed":
    sys.exit(f"MAC6_STATUS {man['status']}")
safety = man["safety"]
for key in ("automatic_promotion", "final_holdout", "private_traces_allowed"):
    if safety[key] is not False:
        sys.exit(f"MAC6_SAFETY {key}={safety[key]!r}")
if safety["worker_kind"] != "deterministic_fake_worker_required":
    sys.exit(f"MAC6_WORKER {safety['worker_kind']}")
if man["evaluation_contract"]["candidate_may_access_final_holdout"] is not False:
    sys.exit("MAC6_HOLDOUT_ACCESS")
if man["evaluation_contract"]["candidate_may_edit_criteria"] is not False:
    sys.exit("MAC6_CRITERIA_EDIT")
specs = man["specialists"]
if len(specs) < 1:
    sys.exit("MAC6_SPECIALISTS empty")
for spec in specs:
    if spec["route_enabled"] is not False or spec["inference_calls_made"] is not False:
        sys.exit(f"MAC6_ROUTE {spec.get('id')} enabled")
    if spec["qualification"] != "unqualified":
        sys.exit(f"MAC6_QUAL {spec.get('id')} {spec['qualification']}")
hold = pins["development_holdout_set_sha256"]
if len(hold) != 64 or any(c not in "0123456789abcdef" for c in hold):
    sys.exit("MAC6_HOLDOUT_PIN")
print("MAC6_FIXTURE_PASS")
PY
