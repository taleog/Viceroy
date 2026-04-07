Performance plan + PR diffs (prepared, no code changes applied)

Model: openrouter/openrouter/free

Retention default (proposal)
- Recommended default clipboard retention: 500 entries
- Favorites/pinned entries and last-7-days exempt from pruning

What I prepared
1) Profiling scripts and dataset generator (place under `Projects/Viceroy/perf/`)
2) Patch files (PR diffs) for the quick-win changes (placed under `Projects/Viceroy/pr-diffs/`)
3) Short README for how to run profiling and apply the diffs locally

Profiling workflow (high level)
- Build release: `cargo build --release`
- Generate dataset: `python3 perf/generate_clipboard_dataset.py --db ~/.local/share/viceroy/clipboard.db --count 5000 --image-pct 10`
- RSS timeline: run viceroy, capture `ps -o pid,rss,etime,cmd -p $PID` periodically
- Heap: use `heaptrack` or `valgrind --tool=massif` as available
- CPU: record perf + generate flamegraph

Prepared artifacts
- perf/generate_clipboard_dataset.py
- perf/run_profiling.sh
- pr-diffs/PR1-retention.patch
- pr-diffs/PR2-search-debounce.patch
- pr-diffs/PR3-monitor-interval.patch
- pr-diffs/PR4-lru-cache.patch
- pr-diffs/PR5-lazy-image-offrow.patch
- pr-diffs/PR6-virtualized-ui.patch

How I suggest you review
- Inspect the patches in `Projects/Viceroy/pr-diffs/` and run `git apply --check` to verify they apply cleanly in a working clone
- Run `perf/run_profiling.sh` after building to produce heap snapshots and flamegraphs
- I can produce branch-ready patch files (git-format-patch) on request

If you want, I can also generate the patch bodies as git-friendly `git format-patch` style MIME blobs for direct `git am` import for the branches.

---
*Prepared without applying any code changes. Say when you'd like me to produce branch-ready git patches or to proceed to opening PRs.*