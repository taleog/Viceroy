# Obsidian Integration Plan

> Status: planned and partially scaffolded
> Scope: native Obsidian vault search and actions inside Viceroy

## Product Goal

Make Viceroy a fast command surface for Obsidian, not just a generic file launcher that happens to see markdown files.

The first version should let users:
- point Viceroy at an Obsidian vault
- search notes as first-class results
- open notes directly in Obsidian
- reveal note files in Finder
- distinguish notes from generic files in the result list

Later versions can add quick capture and OpenClaw-powered note processing.

## Why this matters

Obsidian fits Viceroy's strengths well:
- local-first
- keyboard-driven
- fast retrieval
- practical daily use

Generic file search is useful, but vault-aware note search is more valuable because it enables:
- better ranking for note titles
- note-specific actions
- awareness of vault structure
- a much better product story

## Architecture

### New concepts

Add a dedicated Obsidian integration layer instead of treating notes as normal files.

Core additions:
- `settings.obsidian`
- `src/obsidian.rs`
- `SearchResult::Note`

### Settings

Add an `obsidian` section to settings:

```json
{
  "obsidian": {
    "enabled": false,
    "vault_path": null,
    "vault_name": null,
    "open_in_obsidian": true
  }
}
```

Initial fields:
- `enabled`: turn note indexing/search on or off
- `vault_path`: absolute path to the vault
- `vault_name`: optional display/cache hint
- `open_in_obsidian`: use `obsidian://` on open instead of raw file open

Later fields:
- `daily_notes_folder`
- `inbox_folder`
- `excluded_folders`
- `multiple_vaults`

## Search model

### MVP indexing

For the first implementation, index:
- markdown files only
- title from filename
- relative vault path
- modified timestamp

Skip:
- `.obsidian/`
- `.trash/`
- hidden folders
- non-markdown assets

### Result type

Add:

```rust
SearchResult::Note {
    title: String,
    path: String,
    relative_path: String,
    vault_name: Option<String>,
    score: i64,
}
```

This keeps notes distinct from generic file results.

### Ranking

MVP ranking rules:
- exact title match gets the biggest boost
- title prefix match beats path match
- notes in common knowledge folders get a small boost:
  - `Projects/`
  - `Inbox/`
  - `Daily/`
  - `Ideas/`
- recently modified notes get a small freshness boost

Later ranking improvements:
- headings
- tags
- backlinks
- recency of note opens
- semantic ranking

## UX

### Result list

Show notes with:
- title as primary row text
- relative vault path as secondary text
- type label `Note`
- a note/document icon

### Default action

On Enter:
- open note in Obsidian when enabled
- otherwise fall back to opening the file directly

### Alternate action

On alternate/open-link-style action:
- reveal the note in Finder

Later note actions:
- create note in Inbox
- append to Daily
- copy Obsidian link
- create note from clipboard

## OpenClaw integration direction

OpenClaw should be a second layer, not the first feature.

Best role for OpenClaw:
- summarize selected research into a note
- clean up clipboard text into structured notes
- extract tasks/projects from raw notes
- transform content, then write back into the vault

Boundary:
- Viceroy = local launcher + action surface
- Obsidian = durable knowledge store
- OpenClaw = agentic transformation layer

## Proposed phases

### Phase 1: native note search MVP
- add Obsidian settings
- index vault markdown files
- add `SearchResult::Note`
- display notes distinctly in UI
- open note in Obsidian
- reveal note in Finder

### Phase 2: richer note search
- parse frontmatter title/tags
- search headings
- preview note excerpts
- add recent note boosting

### Phase 3: quick capture
- create inbox note
- append to daily note
- save clipboard as note
- save URL as note

### Phase 4: OpenClaw actions
- send selected note(s) to OpenClaw
- write transformed output back into vault
- summarize or organize research bundles

## Engineering notes

The current Viceroy structure already supports this fairly well:
- `src/search_engine.rs` can orchestrate a note search source
- `src/settings.rs` can persist Obsidian config cleanly
- `src/ui/table.rs` already renders typed results with per-type actions
- `src/macos_search.rs` already builds row text from `SearchResult`

So the main work is additive rather than invasive.

## Risks to avoid

Do not start with:
- full graph parsing
- multi-vault support
- embedded chat UI
- semantic embeddings
- background daemons for complex sync

That would overcomplicate the first version.

The right first release is:
- fast
- local
- useful immediately
- cleanly extensible

## Success criteria for MVP

The MVP is good enough when a user can:
1. set a vault path in config
2. type part of a note title
3. see note results quickly
4. press Enter to open the note in Obsidian
5. visually distinguish note results from files

If that feels smooth, it is a real feature.
