# Current Issues & Status

## Recently Fixed ✅
- **Clipboard paste determinism** - Extended programmatic write detection window from 5s to 10s, added explicit app activation before Cmd+V
- **Duplicate clipboard entries** - Now checks last 5 entries instead of just 1, with app-agnostic comparison
- **Permission prompts on startup** - Disabled file search by default; no filesystem access unless explicitly enabled
- **Tab navigation** - Added Tab key to toggle between search results and clipboard history views

## Known Limitations & Performance Issues

### Tab Key Responsiveness
- Tab toggle between search and clipboard history works but can feel slow on first transition
- **Root cause**: Clipboard history view initialization requires image decoding and preview rendering
- **Workaround**: Subsequent toggles are faster (cached state)

### File Search
- **Disabled by default** to prevent macOS permission prompts on app launch
- Users must explicitly enable with `VICEROY_FALLBACK_FS=1` environment variable
- Spotlight search (`mdfind`) doesn't work reliably on many systems
- Fallback filesystem indexing only searches: Documents, Downloads, Desktop (limited scope)

### Search Performance
- Large result sets may impact scrolling performance
- Search results updating can lag on slower machines
- Initial clipboard history load times depend on clipboard size and image complexity

### Animations
- Spring animations defined in helpers but not yet integrated into window show/hide
- Could improve visual feedback for view transitions

## Technical Details

### How Clipboard Works
1. Clipboard monitor polls every 200ms for changes
2. Detects programmatic writes for 10 seconds after paste
3. Compares against last 5 entries to detect duplicates
4. Skips password manager apps (Keychain, 1Password, Bitwarden, etc.)

### Search Pipeline
- Parallel execution of app, file, clipboard, command, calculator, and emoji searches
- Smart ranking based on match quality and query context
- Top 50 results returned, capped per category for diversity
- Latency logging available with `RUST_LOG=debug`

### UI Modes
- **Search**: Default mode showing search results
- **ClipboardHistory**: Alternative view for browsing clipboard history
- **Settings**: Configuration panel

## Environment Variables for Development

```bash
# Enable filesystem fallback search (scans Documents/Downloads/Desktop)
VICEROY_FALLBACK_FS=1

# Enable debug logging to see search performance metrics
RUST_LOG=debug

# Disable automatic update checks
VICEROY_UPDATE_CHECK_DISABLED=1
```

## Future Improvements

### High Priority
- Implement persistent lightweight file index (cached to disk) to avoid permission prompts and improve search speed
- Add automated tests for clipboard paste flow and deduplication
- Integrate spring animations into window transitions for better visual feedback

### Medium Priority
- Optimize clipboard history view loading (lazy load images, cache previews)
- Add contact search, tab search, and OCR for clipboard images
- Implement basic keyboard shortcuts help UI

### Lower Priority
- Color picker integration
- Quick actions/macros
- Audio file search
- Notes/bookmarks integration

## Debugging Tips

### Check if file search is enabled
```bash
echo $VICEROY_FALLBACK_FS
```

### View search performance logs
```bash
RUST_LOG=search_engine=info /Applications/Viceroy.app/Contents/MacOS/viceroy
```

### Test clipboard detection
- Copy something, wait 3 seconds, check app's clipboard history
- Try pasting from history - should not create duplicate

### Restart with fresh state
```bash
rm ~/.config/viceroy/clipboard.db
pkill Viceroy
open /Applications/Viceroy.app
```
