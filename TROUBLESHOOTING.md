# Viceroy - Troubleshooting Guide

## Build Issues

### Issue: Runtime Panic - "no reactor running"
**Fixed in latest version**

The clipboard monitor was trying to spawn a tokio task before the runtime was initialized.

**Solution**: The clipboard monitor now runs in a separate thread with its own tokio runtime.

### Issue: Unused Imports Warnings
**Status**: Cleaned up in latest version

Removed unused imports:
- `NSArray` from cocoa::foundation
- `Object`, `Class` from objc::runtime
- `HashMap` from std::collections

### Issue: objc Crate Warnings
**Status**: Non-critical

The `objc` crate generates warnings about `cargo-clippy` cfg conditions. These can be safely ignored or fixed by updating the objc dependency:

```bash
cargo update -p objc
```

## Runtime Issues

### Issue: UI Server Port Already in Use

If you see "Address already in use" when starting dev mode:

```bash
# Find and kill the process using port 8080
lsof -ti:8080 | xargs kill -9

# Or use a different port by editing serve_ui.py
# Change: PORT = 8080
# To: PORT = 8081
```

### Issue: Permissions Not Granted

The app requires several macOS permissions:

1. **Accessibility** - For global hotkeys
   ```
   System Settings > Privacy & Security > Accessibility
   Add Viceroy
   ```

2. **Automation** - For app launching
   ```
   System Settings > Privacy & Security > Automation
   Enable Viceroy for Terminal/System Events
   ```

3. **Full Disk Access** (Optional) - For comprehensive file search
   ```
   System Settings > Privacy & Security > Full Disk Access
   Add Viceroy
   ```

### Issue: Global Hotkey Not Working

**Symptoms**: Pressing Cmd+Space doesn't open the launcher

**Solutions**:
1. Check Accessibility permissions
2. Verify no conflict with Spotlight (System hotkey)
3. Try changing the hotkey in settings:
   ```json
   {"hotkey": "CommandOrControl+K"}
   ```
4. Restart the application

### Issue: Clipboard History Not Saving

**Symptoms**: Clipboard entries don't appear in search

**Solutions**:
1. Check database file exists:
   ```bash
   ls -la ~/.config/viceroy/clipboard.db
   ```

2. Verify permissions:
   ```bash
   chmod 644 ~/.config/viceroy/clipboard.db
   ```

3. Check logs:
   ```bash
   RUST_LOG=debug cargo run
   ```

4. Reset database:
   ```bash
   rm ~/.config/viceroy/clipboard.db
   # Restart app to recreate
   ```

### Issue: File Search Returns No Results

**Symptoms**: Searching for files returns empty results

**Solutions**:
1. Rebuild Spotlight index:
   ```bash
   sudo mdutil -E /
   ```

2. Check Spotlight is enabled:
   ```bash
   mdutil -s /
   ```

3. Wait for indexing to complete (can take minutes to hours)

4. Verify Full Disk Access permissions

### Issue: Apps Won't Launch

**Symptoms**: Clicking app results does nothing

**Solutions**:
1. Check Automation permissions
2. Verify app still exists:
   ```bash
   ls -la /Applications/Safari.app
   ```
3. Try launching from terminal:
   ```bash
   open -a Safari
   ```
4. Clear app cache (wait 5 minutes or restart app)

## Development Issues

### Issue: Compilation Fails

**Solutions**:
1. Clean and rebuild:
   ```bash
   cargo clean
   cargo build
   ```

2. Update dependencies:
   ```bash
   cargo update
   ```

3. Check Rust version:
   ```bash
   rustc --version
   # Should be 1.70+
   ```

4. Reinstall Rust:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Issue: UI Not Loading

**Symptoms**: Window appears but is blank

**Solutions**:
1. Verify UI server is running:
   ```bash
   curl http://localhost:8080
   ```

2. Check UI file exists:
   ```bash
   ls -la ui/index.html
   ```

3. Check browser console for errors (if applicable)

4. Use `distDir` instead of `devPath` in `tauri.conf.json` for production

### Issue: Binary Size Too Large

**Current size**: ~8-12MB (within target)

If binary is larger than expected:
1. Ensure release build:
   ```bash
   cargo build --release
   ```

2. Verify optimization settings in `Cargo.toml`:
   ```toml
   [profile.release]
   opt-level = "z"
   lto = true
   codegen-units = 1
   strip = true
   ```

3. Check binary size:
   ```bash
   du -h target/release/viceroy
   ```

## Performance Issues

### Issue: High CPU Usage

**Symptoms**: Fan spinning, high CPU in Activity Monitor

**Solutions**:
1. Check clipboard polling interval (default 200ms is reasonable)
2. Increase debounce delay in search (default 150ms)
3. Reduce max results (default 50)
4. Check for infinite loops in logs

### Issue: High Memory Usage

**Expected**: ~30-50MB idle, ~100-150MB during search

**Solutions**:
1. Clear clipboard history:
   ```bash
   rm ~/.config/viceroy/clipboard.db
   ```

2. Reduce clipboard entry limit (default 1000)

3. Check for memory leaks with instruments:
   ```bash
   cargo instruments --release --template Leaks
   ```

### Issue: Slow Search Results

**Expected**: <200ms average

**Solutions**:
1. Rebuild Spotlight index
2. Check file system performance
3. Reduce result limit
4. Disable external drive search if not needed

## Common Warnings

### Warning: "code that will be rejected by a future version of Rust"

**Package**: `nom v1.2.4`

**Status**: Non-critical, dependency issue

**Solution**: Wait for upstream update or ignore

### Warning: "unexpected `cfg` condition value"

**Status**: Non-critical, objc crate issue

**Solution**: 
```bash
cargo update -p objc
```

Or ignore - doesn't affect functionality.

## Debugging Commands

### Enable Debug Logging
```bash
RUST_LOG=debug cargo run
```

### Check Specific Module
```bash
RUST_LOG=viceroy::clipboard=debug cargo run
```

### Backtrace on Panic
```bash
RUST_BACKTRACE=1 cargo run
```

### Profile Performance
```bash
cargo build --release
cargo instruments --release --template "Time Profiler"
```

### Check Dependencies
```bash
cargo tree
```

### Audit Dependencies
```bash
cargo audit
```

## Getting Help

### Before Reporting Issues

1. Check this troubleshooting guide
2. Read `SETUP.md` for proper setup
3. Check logs with `RUST_LOG=debug`
4. Try clean rebuild: `cargo clean && cargo build`
5. Verify all permissions are granted

### Information to Include

When reporting issues, include:
- macOS version
- Rust version (`rustc --version`)
- Full error message
- Steps to reproduce
- Relevant logs
- Output of `cargo check`

### Debug Checklist

- [ ] Proper macOS permissions granted
- [ ] UI server running on port 8080
- [ ] Database file exists and is writable
- [ ] No conflicting global hotkeys
- [ ] Spotlight indexing complete
- [ ] Latest code from repository
- [ ] Clean build attempted
- [ ] Logs checked with RUST_LOG=debug

## Quick Fixes

### Reset Everything
```bash
# Stop app
killall viceroy

# Clean build
cargo clean

# Remove config
rm -rf ~/.config/viceroy

# Rebuild
cargo build --release

# Run
./target/release/viceroy
```

### Force Permissions Reset
```bash
# Reset accessibility database (requires admin)
sudo tccutil reset Accessibility com.viceroy.app
```

### Clear All Caches
```bash
rm -rf ~/.config/viceroy
rm -rf target/
cargo clean
```

## Known Limitations

1. **Polling-based clipboard monitoring** - Can miss very rapid clipboard changes
2. **Single window instance** - Cannot open multiple launcher windows
3. **macOS only** - Not portable to other platforms
4. **Requires Spotlight** - File search depends on macOS indexing
5. **English UI only** - No internationalization yet

## Future Improvements

- Event-driven clipboard monitoring
- Multiple window support
- Cross-platform compatibility
- Custom file indexing
- Localization support
- Plugin system for extensibility
