#!/usr/bin/env bash
# nus.5: full cli-pool recovery batch (evidence pass over the ~1,431
# description-app-smelling cli entries in results/cli-recovery-pool.txt).
#
# Runs evidence-all.js in chunks (LIMIT=100) until the pool is exhausted or
# the local time passes DEADLINE (default 07:30). Safe to kill and restart:
# results/evidence-cli.jsonl is append-only and already-seen entries are
# skipped on resume; recovered (rich/basic) rows promote to class app via
# classify-evidence.js, which must be re-run after the batch.
#
# Usage: ./nus5-batch.sh [DEADLINE_HH:MM]
set -u
cd "$(dirname "$0")"
DEADLINE="${1:-07:30}"
POOL="results/cli-recovery-pool.txt"
RUNLOG=/tmp/nus5-batch.log

echo "nus.5 batch started $(date) — deadline ${DEADLINE}" | tee "$RUNLOG"
while :; do
  now=$(date +%H:%M)
  if [[ "$now" > "$DEADLINE" ]]; then
    echo "deadline reached at $now — stopping (resumable)" | tee -a "$RUNLOG"
    break
  fi
  out=$(EV_OUT=results/evidence-cli.jsonl node evidence-all.js 100 --attrs-from "$POOL" 2>&1)
  echo "$out" | tee -a "$RUNLOG"
  if echo "$out" | grep -q "0 still queued"; then
    echo "pool exhausted — done" | tee -a "$RUNLOG"
    break
  fi
  # count remaining (+1 line carries 'still queued of N')
  remaining=$(echo "$out" | grep -oE '[0-9]+ still queued' | grep -oE '^[0-9]+' || echo "?")
  echo "$(date +%H:%M) — chunk done, ~${remaining} still queued" | tee -a "$RUNLOG"
done
echo "nus.5 batch finished $(date)" | tee -a "$RUNLOG"
