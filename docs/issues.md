# Current Issues

- Clipboard paste from history still reappears as a duplicate and Enter-triggered paste is unreliable (focus timing / monitor pause not sufficient).
- File search prompts for folder access when fallback walker is enabled; Spotlight remains broken so results are sparse without fallback.
- File search performance regressed compared to original fast Spotlight flow (initial fallback index cost on first query).
- Animations are subtle; bounce/fade may not be visible or feel laggy on show/hide.
- Scrolling sometimes still feels constrained when result counts are large (table content height/layout).

# Suspected Root Causes / Leads

- Paste: focus not returning to target app before Cmd+V; monitor pause too short; programmatic write detection may miss some cases.
- Duplicates: monitor polling interval and programmatic detection window may be misaligned; images may hash differently after encode/decode.
- Permissions: fallback walker traverses user folders; macOS privacy prompts if not pre-approved.
- File results: Spotlight indexing unavailable; fallback gated behind env flag; need cached user-index to avoid prompts.
