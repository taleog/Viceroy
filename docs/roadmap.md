# Viceroy Development Roadmap

> Last updated: 2026-03-28

## Release Status: v0.1.0-alpha.1 (Private Alpha)

### Version Scheme
- **Alpha** (0.x.y-alpha.z): Early development, breaking changes expected
- **Beta** (0.x.y-beta.z): Feature complete for milestone, bug fixes only
- **Release** (x.y.z): Stable releases

### ✅ Completed (v0.1.0-alpha.1)
- [x] Clipboard paste determinism - app activation + extended write detection window
- [x] Duplicate clipboard elimination - check last 5 entries with app-agnostic comparison  
- [x] Privacy prompt fix - disabled file search by default
- [x] Tab navigation - toggle between search and clipboard history views
- [x] Search latency metrics - performance logging for all sources
- [x] Animation helpers - slide+spring show and fade+scale hide (defined, not yet integrated)
- [x] Windows desktop app - native Windows shell sharing the Rust backend
- [x] Self-hosted clipboard sync server - `viceroy-sync-server` with SQLite storage and WebSocket fan-out
- [x] Cross-device sync settings - macOS and Windows settings UIs with persisted sync configuration
- [x] Settings compatibility migration - older flat sync keys migrate into the nested `sync` config section

### ⏳ In Progress / Blocked
- [ ] **Tab responsiveness optimization** - clipboard view loads slowly on first access
  - Needs: Image preview caching, lazy loading of clipboard items
  - Impact: Medium - affects UX but Tab still works
  
- [ ] **Persistent file index** - cached index to avoid permission prompts
  - Blocked on: Design for secure, lightweight cache format
  - Needs: Persistent storage, periodic background updates
  - Impact: High - would restore fast file search

## Immediate Next Steps (Next Sprint)

### High Priority
1. **Optimize clipboard history view performance**
   - Implement lazy loading of clipboard entries
   - Cache image previews instead of decoding on each view toggle
   - Use background thread for image processing
   - Target: Tab toggle should feel instant

2. **Persistent file index implementation**
   - Design lightweight index format (SQLite or JSON cache)
   - Index Documents/Downloads/Desktop on background thread
   - Periodically refresh without blocking UI
   - Restore fast file search without permission prompts
   - Target: File search working as fast as original Spotlight

3. **Add automated tests**
   - Clipboard paste and duplicate detection tests
   - Search ranking tests
   - Integration tests for window focus/paste flow
   - Target: 80%+ coverage of critical paths

### Medium Priority
4. **Integrate animation improvements**
   - Use slide+spring animation for window show
   - Use fade+scale animation for window hide
   - Add row hover/selection visual feedback
   - Target: Make transitions feel smooth and responsive

5. **Expand search sources**
   - Add macOS Contacts search
   - Add Safari/Chrome tab search
   - Add clipboard image OCR (via Vision framework)
   - Target: Richer search results for power users

6. **Settings UI improvements**
   - Add sync connection test button
   - Add file search enable/disable toggle
   - Add keyboard shortcuts help panel
   - Add clipboard retention policy settings
   - Target: User-friendly configuration

## Medium-Term Goals (1-2 Months)

### Performance & Stability
- [ ] Achieve sub-100ms search latency for common queries
- [ ] Ensure no freezing on Tab or search mode changes
- [ ] Handle very large clipboards (1000+ items) smoothly
- [ ] Reduce memory footprint

### Feature Completeness
- [ ] Full contact/email search integration
- [ ] Browser tab search across Safari/Chrome/Firefox
- [ ] Quick actions/macro support
- [ ] Custom keyboard shortcuts

### Quality
- [ ] Comprehensive test suite (unit + integration)
- [ ] Performance benchmarking framework
- [ ] Crash logging and error reporting
- [ ] User analytics (opt-in)
- [ ] Sync device revocation and last_seen metadata

## Long-Term Vision (3+ Months)

### Advanced Features
- [ ] Color picker with history
- [ ] Code snippet search and formatting
- [ ] Web search engine switcher
- [ ] Custom search plugins/extensions
- [ ] Shared/team sync auth model beyond a single bearer token

### Platform Support
- [ ] iOS/iPadOS companion app
- [ ] Cloud backup of clipboard history
- [ ] Team/shared clipboard features

### Optimization
- [ ] Native assembly optimizations where beneficial
- [ ] GPU acceleration for image processing
- [ ] ML-based ranking for personalized search

## Known Technical Debt

1. **File search disabled** - needs persistent index design
2. **Tab responsiveness** - clipboard view needs async loading
3. **Animation integration** - helpers exist but not used
4. **Error handling** - limited user-facing error messages
5. **Testing** - minimal automated test coverage
6. **Documentation** - keep branch capabilities and setup docs aligned across platforms
7. **Sync auth model** - current personal setup uses a shared bearer token; per-device revocation is still future work

## Success Metrics

- App loads and responds without permission prompts ✅
- Search results in <500ms for typical queries
- Tab toggle feels instant (<200ms)
- File search as fast as original Monarch (if enabled)
- Zero crashes on normal usage
- >80% test coverage for critical code paths

## Known Limitations (Won't Fix v1.0)

- Spotlight search doesn't work reliably (system limitation)
- Clipboard monitoring limited to 200ms polling (system limitation)
- No web-based UI (by design - native only)
- No polished Linux desktop UI yet (CLI fallback only outside macOS/Windows)
- Clipboard history limited to ~10K items (storage/performance)
- Sync is currently personal/self-hosted first; multi-tenant auth and revocation are future work

## Links & Resources

- GitHub: https://github.com/taleog/Viceroy
- Original Monarch: https://monarchapp.com/
- Rust + macOS: https://github.com/ryanmcgrath/cacao
- AppKit Reference: https://developer.apple.com/documentation/appkit

## Contributing

Interested in helping? Areas of focus:
- Performance optimization (search, clipboard view)
- Test suite development
- Feature implementation (contacts, tabs, OCR)
- Documentation and examples
