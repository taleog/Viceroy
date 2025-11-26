# Viceroy Features - Complete Reference

## Overview
Viceroy is a productivity launcher that appears on top of everything. It's an extension of macOS that allows performing tasks without switching apps or workspaces.

---

## Core Features Implemented ✅

### 1. **App Launcher Mode** (Default)
- Lightning fast app search (150x faster than Spotlight)
- Fuzzy search with smart ranking
- Launch applications instantly

### 2. **Search Modes with Tab Cycling**
- 🔍 **Search All** (default, always enabled)
- 📱 **Apps** (apps only)
- 📄 **Files** (files only)
- 📋 **Clipboard** (clipboard history)
- 🧮 **Calculator** (math expressions)
- 😀 **Emoji** (emoji picker)

### 3. **Mode Management**
- Tab key to cycle through modes
- Double-click to disable modes (grays out)
- Click disabled mode to re-enable
- Order and preferences saved to localStorage

### 4. **Clipboard History** ✅ (Basic)
- Stores everything copied
- Searchable history
- Shows preview and app name

### 5. **Calculator** ✅
- Math expressions
- Multiple formats (decimal, hex, binary, percentage)

### 6. **Emoji Picker** ✅
- Search emojis with `:` prefix
- ~200 curated emojis
- Search by name and keywords

### 7. **Dictionary** ✅
- Define words with "define" prefix
- Opens macOS Dictionary.app

### 8. **Web Search** ✅
- Multiple engines (Google, DuckGo, YouTube, GitHub)
- URL encoding

### 9. **File Search** ✅ (Basic)
- Fuzzy file search
- 20 results limit in All mode, 50 in Files mode

### 10. **System Commands** ✅
- Various system actions

### 11. **Smart Ranking** ✅
- Exact match priority
- Prefix matching boost
- Common apps prioritization
- File type awareness
- Query length scaling

---

## Features to Implement 🚧

### **Clipboard History (Advanced)**
**Priority: HIGH**

#### Navigation & Actions
- ✅ Up/down arrow navigation
- ❌ **Autopaste with ENTER** - paste to active app
- ❌ **Recopy with ⌘+C** - copy without pasting
- ❌ **Pin items with ⌘+P** - keep at top
- ❌ **Delete with CTRL+Backspace**
- ❌ **Rename with ⌘+R**
- ❌ **Edit text entries** (hover to show Edit button)
- ❌ **Multi-select pasting** (SHIFT + arrows)

#### Filtering & Organization
- ❌ Filter by: Application, Text, Links/URLs, Images
- ❌ Pinned items always on top
- ❌ Can't delete pinned items until unpinned

#### Privacy & Security
- ❌ **Pause clipboard** (⌘+SHIFT+P)
- ❌ **Ignore password managers**: Keychain, 1Password, Bitwarden, etc.

---

### **Viceroy Notes**
**Priority: HIGH**

- ❌ Create note with ⌘+N
- ❌ Save note with ⌘+S
- ❌ Open note with ⌘+O
- ❌ SHIFT+ENTER to create non-existent note
- ❌ Markdown support (plain markdown files)
- ❌ Configurable save location
- ❌ Works with Obsidian (same folder)
- ❌ Default hotkey: ⌘+.

**Note**: Tables, Images, Links not fully supported yet in Viceroy

---

### **Superlinks** 
**Priority: MEDIUM**

Complex but powerful feature for custom shortcuts:

#### Core
- ❌ Create custom shortcuts to apps/websites
- ❌ Search "Create Superlink" to make new
- ❌ "All Superlinks" command to view all
- ❌ Delete with CTRL+Delete
- ❌ Edit with ⌘+E

#### Advanced Features
- ❌ **Parameters**: `{user}` in URL becomes fillable field
- ❌ **Default values**: `{user:rmdashrfv}` - fallback if empty
- ❌ **Optional params**: `{repo?}` - omit if empty
- ❌ **Parameter reordering**: `{[1]language}` sets order
- ❌ **Auto triggers**: typing flows into parameters
- ❌ **Custom icons**
- ❌ **Open with specific app**
- ❌ **Match check**: prevent duplicate tabs
- ❌ **Fallback link**: URL when all params empty
- ❌ **Use Viceroy**: open within Viceroy panel

**Example Superlinks:**
- `https://github.com/{user:rmdashrfv}/{repo?}`
- `https://www.deepl.com/translator#{[3]source:en}/{[2]target:es}/{[1]phrase?}`

---

### **Color Picker**
**Priority: MEDIUM**

- ❌ Access with "Color Picker" or "cp"
- ❌ Pick color with eyedropper (⌘+1)
- ❌ Copy to clipboard (double-click or hover icon)
- ❌ Create palettes (⌘+2)
- ❌ Rename colors (click name or edit icon)
- ❌ View formats (⌘+4): Hex, RGB, HSL
- ❌ Delete colors (trash icon, SHIFT to skip confirm)
- ❌ Search by hex codes in root search
- ❌ RGB format: `(25, 10, 210)` or `25,10,210`
- ❌ Hex format: `#fb4f41`

---

### **File Search (Enhanced)**
**Priority: MEDIUM**

#### Options
- ✅ Root Search (files in main search) - DONE
- ❌ **Custom prefix** (e.g., "f " to search files only)
- ❌ None (no file search)

#### File Navigation
- ❌ **Right arrow** to enter folder
- ❌ Navigate nested folders
- ❌ ENTER on file - open with default app
- ❌ ENTER on folder - open in Finder
- ❌ ⌘+ENTER - reveal in Finder
- ❌ ESC to exit navigation
- ❌ CTRL+C to reset completely

#### Advanced
- ❌ View hidden files in navigation
- ❌ Status bar showing item count
- ❌ Hide files with ⌘+H
- ❌ Shows full file path
- ❌ Excludes config files by default
- ❌ Configurable folders to index
- ❌ iCloud Drive support (planned)
- ❌ External drive support (not planned)

---

### **Instant Send** (>)
**Priority: MEDIUM**

- ❌ Press `>` on file/folder to get actions
- ❌ Move, Copy, Delete options
- ❌ "Open With" for files AND folders
- ❌ ESC to cancel

---

### **Additional Features**

#### Apple Reminders
- ❌ Create reminders from Viceroy
- ❌ Syncs with Apple Reminders app

#### Audio Devices
- ❌ Quick connect to AirPods, Bluetooth devices
- ❌ Switch audio output

#### Kill Process
- ❌ Custom prefix to terminate processes
- ❌ View memory and CPU usage

#### Unit & Currency Conversion
- ❌ `1 lb to grams`
- ❌ `$5 to EUR`
- ❌ Timezone: `12:30pm EST to PDT`

#### Settings
- ❌ Custom themes (light & dark separately)
- ❌ Theme editor for colors, background, text
- ❌ Where Viceroy opens (cursor screen vs primary)
- ❌ Contacts in search (Apple contacts)
- ❌ Web search in browser vs Viceroy panel
- ❌ Keyboard shortcuts manager

#### Other
- ❌ Command history
- ❌ Web bookmarks search (Safari/Chrome/Firefox/Edge)
- ❌ Screen recording controls
- ❌ Volume settings
- ❌ Quick settings access (Wifi, Sound, etc.)
- ❌ Hide items feature (⌘+/)
- ❌ Variables system

---

## UI/UX Features

### Implemented ✅
- Transparent window with vibrancy
- Mode selector with icons
- Tab cycling through modes
- Smooth animations (0.3s cubic-bezier)
- Keyboard hints at bottom
- Active mode indicator
- Smooth result animations
- Empty state
- Loading state

### To Implement ❌
- **Auto-revert**: Close after selection (with exceptions for media)
- **Prefix triggers**: Automatic parameter entry
- **Status bar**: Show result counts, folder info
- **Hover actions**: Edit, Delete, Pin buttons appear
- **Multi-select**: SHIFT+click, SHIFT+arrows
- **Right-click context menus**
- **Inline editing**: Text entries, notes
- **Web panel**: Browse within Viceroy with back/forward/reload
- **Theme customization UI**

---

## Architecture Notes

### Current Structure
```
src/
├── main.rs (Tauri commands, window management)
├── search_engine.rs (unified search with modes)
├── app_launcher.rs (macOS app search via NSWorkspace)
├── file_search.rs (file searching)
├── clipboard.rs (clipboard monitoring & storage)
├── calculator.rs (math evaluation)
├── emoji.rs (emoji database & search)
├── dictionary.rs (macOS dictionary integration)
├── web_search.rs (multi-engine web search)
├── system_commands.rs (system actions)
├── settings.rs (user preferences)
└── database.rs (SQLite for clipboard)

ui/
└── index.html (single-page UI with mode selector)
```

### Key Technologies
- **Tauri 1.5**: Web UI + Rust backend
- **Tokio**: Async runtime
- **Rusqlite**: SQLite for clipboard
- **Arboard**: Clipboard monitoring
- **Cocoa/objc**: macOS NSWorkspace integration
- **Window-vibrancy**: Native blur effects

### Performance Targets
- Binary size: <20MB (current approach)
- Search response: <100ms
- Window open: <150ms
- Smooth 60fps animations

---

## Next Steps (Recommended Priority)

1. **Clipboard History Advanced** (⌘+P pin, ⌘+R rename, autopaste, filtering)
2. **Viceroy Notes** (⌘+. hotkey, markdown editor)
3. **File Navigation** (right arrow to enter folders)
4. **Superlinks** (powerful but complex)
5. **Color Picker** (useful for designers)
6. **Unit/Currency Conversion** (enhance calculator)
7. **Instant Send** (file actions with >)
8. **Audio Devices** (quick switching)
9. **Theme System** (customization)
10. **Additional integrations** (Reminders, bookmarks, etc.)
