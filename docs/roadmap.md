# Near-Term Fixes

- Make clipboard pastes deterministic: explicitly activate previous app before paste, extend pause, and ignore programmatic writes for a longer window; add integration test for Enter-to-paste.
- Eliminate duplicate clipboard entries: compare against last N entries (not just last), include app-name-agnostic check, and skip saves immediately after history paste.
- Stop privacy prompts: disable fallback walker by default, add a one-time user-approved indexer running only in allowed directories, and surface a clear toggle in settings.
- Restore fast file search: prefer Spotlight when available; otherwise use a persistent lightweight index (cached to disk) to avoid per-query scans.

# Improvements

- Make animations visible but lightweight: short slide + spring on show, subtle fade/scale on hide, and row hover/selection affordances.
- Add metrics: log search latency per source and clipboard paste success; expose a debug overlay to verify responsiveness.
- Expand search sources once stability is restored: contacts, tabs, clipboard images OCR, quick actions.
- Add basic automated tests for search ranking, clipboard dedupe, and window focus/paste flow (headless where possible).
