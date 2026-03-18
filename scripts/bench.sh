#!/usr/bin/env bash
# Unlicense — cochranblock.org
# Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

# Baseline latency benchmark for Kova. Run with serve already up.
# Usage: kova serve & sleep 2; ./scripts/bench.sh

set -e
BASE="${KOVA_BENCH_URL:-http://127.0.0.1:3002}"
echo "Benchmarking $BASE"
echo "GET /api/status latency (seconds):"
curl -w '%{time_total}\n' -o /dev/null -s "$BASE/api/status"