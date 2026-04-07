#!/usr/bin/env bash
set -euo pipefail

# Build release
cargo build --release

# Usage: ./run_profiling.sh <path-to-viceroy-binary-args>
# Example: ./run_profiling.sh "--some-ui-flag"

ARGS=${1-""}

BIN=target/release/viceroy
if [ ! -x "$BIN" ]; then
  echo "Build failed or binary not found: $BIN"
  exit 1
fi

# Start the app in background
"$BIN" $ARGS &
PID=$!
echo "Started viceroy (pid=$PID)"

# RSS monitor
echo "Recording RSS to perf/rss.log"
mkdir -p perf
(
  while kill -0 "$PID" 2>/dev/null; do
    ps -o pid,rss,etime,cmd -p "$PID"
    sleep 2
  done
) > perf/rss.log &
RSS_PID=$!

# Suggest manual profiling steps if tools are not installed
cat <<EOF
Now the app is running. Recommended profiling commands (run in another shell):

# Heaptrack (preferred on Linux)
# sudo apt install heaptrack
# heaptrack --output=perf/heaptrack.%p.gz $BIN $ARGS
# heaptrack_print perf/heaptrack.<pid>.gz > perf/heap-summary.txt

# Valgrind massif (if heaptrack unavailable):
# valgrind --tool=massif --time-unit=ms $BIN $ARGS
# ms_print massif.out.* > perf/massif.txt

# Flamegraph / perf (requires perf and FlameGraph scripts):
# sudo perf record -F 99 -g -- $BIN $ARGS
# sudo perf script | ./stackcollapse-perf.pl | ./flamegraph.pl > perf/flame.svg

When finished, ctrl-c to stop this script (it will kill the rss monitor). Logs will be in perf/
EOF

wait $PID || true
kill $RSS_PID 2>/dev/null || true
