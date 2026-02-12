# Specification - XML Reader

## Purpose

A desktop application built with **Tauri** (Rust + Svelte 5 + Tailwind CSS) that reads large XML files (~2GB), searches for elements, and displays their XPath.

## Requirements

### Platform
- The app runs on **Windows** (primary target).

### Performance
- Must remain performant on large files (~2GB).
- Files are **never** loaded entirely into RAM — the backend uses streaming (Seek-based `BufReader`) to read only the needed chunks.
- Search can be **cancelled** mid-operation to avoid indefinite blocking on very large files.

### File Generation
- Built-in utility to generate sample XML files for testing.
- Configurable **size** (in MB, up to 2048) and **maximum depth** of the tree.
- Generated files are stored in the app's local data directory (`appLocalDataDir`).
- Element names progress logically (e.g., `Alpha_L1`, `Beta_L2`, etc.) with level indicators.
- Each element includes `guid`, `id`, and `name` attributes for searchability.
- Size presets available: Small (10MB), Medium (100MB), Large (500MB), Max (2GB).

## Design

### Landing Page
- A **hero page** with a central drag-and-drop zone for XML files.
- A **recent files** panel on the right showing the last 10 opened files (persisted to `localStorage`).
- Each recent file entry shows the filename and full path, with a remove button on hover.
- Tauri native drag-drop events are used for reliable file path resolution.

### Viewer Layout
- When a file is selected, a full-screen viewer replaces the landing page.
- **Sticky header** at the top with:
  - **Home button**: returns to the landing page (closes the file).
  - **File name**: displayed as basename only (truncated for long names), with full path on hover.
  - **Navigation buttons**: Jump to first / last element in the root.
  - **Search bar**: compact input field with "Find tag..." placeholder and a "searching..." pulse animation during active search.
  - **Cancel button**: appears (red ✕) during active search to abort the operation.
  - **Prev / Next buttons**: navigate between search matches (chevron icons).
  - **XPath display box**: shows the current element's XPath path.
    - **Hover**: tooltip with the full XPath string.
    - **Click**: copies the XPath to clipboard.
    - **Visual states**: green border on successful copy, blue ring while loading, red border + "Not found" warning (auto-dismisses after 2 seconds).
- **Content area**: three-panel view when an element is active:
  - Dimmed context **before** the element (~2KB).
  - **Active element** highlighted with amber background and left border, pretty-printed, with a copy button on hover.
  - Dimmed context **after** the element (~2KB).
  - Falls back to a single-chunk raw view before any search is performed.

### Syntax Highlighting
- **Tags**: blue (`#60a5fa`), bold.
- **Attributes**: purple (`#c4b5fd`).
- **Values**: green (`#86efac`).
- **Comments**: gray, italic.

## Search

- Case-insensitive matching.
- Searches by:
  - **Tag name**
  - **`id` attribute**
  - **`guid` attribute**
  - **`name` attribute**
- Supports "Find Next" (continues from last match offset + 1) and "Find Previous" (restarts from offset 0).
- Results include the full XPath, the element's text, and ~2KB of surrounding context.
- If not found, a red warning is shown inline in the XPath box for 2 seconds (no alert dialog).
- Long-running searches can be cancelled via a dedicated cancel button backed by an `AtomicBool` flag on the backend.

## Testing Workflow

1. Generate a sample file from the landing page (choose size and depth).
2. The generated file opens automatically in the viewer.
3. Search for an element by tag name, id, guid, or name.
4. Verify the XPath is displayed in the header.
5. Use Prev/Next to navigate matches.
6. Use Jump to First / Jump to Last for boundary navigation.
7. Copy XPath or element XML via click interactions.