# Design Documentation - XML Reader

## Overview

A high-performance Desktop XML Reader built with Tauri (Rust + Svelte 5), designed to handle large files (up to 2GB) with a modern, dark-themed interface.

## Layout & UI Requirements

### 1. Global Theme

- **Style**: Dark Mode with blue accent colors.
- **Background**: `bg-gray-950` for main content, `bg-gray-900` for panels/headers.
- **Text**: `text-gray-100` (primary), `text-gray-400` (secondary), `text-gray-500`/`text-gray-600` (muted).
- **Font**: System sans-serif for UI, monospace (`font-mono`) for XML content, XPath, search input, and file names.
- **Borders**: `border-gray-700` default, `border-gray-800` subtle dividers.
- **Transitions**: All interactive elements use `transition-colors` for smooth state changes.

### 2. Landing Layout

The landing page is a centered flex container with two sections side-by-side.

#### DropZone (Left)

- **Container**: Max width `max-w-lg`, dashed border (`border-2 border-dashed`), rounded corners (`rounded-2xl`).
- **Idle state**: `border-gray-700 bg-gray-900/60` with a 📄 emoji icon.
- **Drag-over state**: `border-blue-400 bg-blue-500/10 scale-[1.02] shadow-xl shadow-blue-500/10` with a 📂 emoji icon.
- **Drop handling**: Uses Tauri's native `onDragDropEvent` for reliable file path resolution on Windows.
- **Generation controls** (below divider):
  - Two number inputs: **Size (MB)** and **Depth**.
  - Four preset buttons: Small (10MB/d2), Medium (100MB/d3), Large (500MB/d4), Max (2GB/d5).
  - A full-width blue **Generate & Open** button with loading state animation.
  - Inputs styled with `bg-gray-800 border-gray-700` and blue focus ring.

#### Recent Files Panel (Right)

- **Container**: Fixed width `w-72`, `max-h-120`, `bg-gray-900/60`, rounded corners, overflow scroll.
- **List items**: Each entry shows:
  - A 📄 icon that turns blue on hover.
  - **File name** (bold, truncated) and **full path** (tiny, muted, truncated).
  - A **remove button** (✕) that appears on hover, with red hover color.
- **Persistence**: Last 10 files stored in `localStorage`.

### 3. Main Viewer Layout

A full-screen flex column layout (`h-screen`) with a sticky header and scrolling content area.

#### Header Area

- **Position**: Sticky top (`sticky top-0`), `z-10`, height `h-14`.
- **Style**: Glassmorphism — `bg-gray-900/90 backdrop-blur-md border-b border-gray-800`.
- **Layout**: Horizontal flex with `gap-3`, vertically centered items.

**Components (left to right):**

1. **Home Button**: `w-7 h-7`, custom SVG icon, navigates back to landing page.
2. **Separator**: `text-gray-700` pipe character.
3. **File Name**: `font-mono text-xs text-gray-400`, truncated (`max-w-30`), full path shown via `title` attribute.
4. **Separator**: Pipe character.
5. **Jump to First**: Custom icon button, navigates to first child element of root.
6. **Jump to Last**: Custom icon button, navigates to last child element of root.
7. **Search Bar**:
   - Container: `w-56`, `border border-gray-700 rounded-sm`, `bg-gray-800/50`.
   - Focus state: `border-blue-500 ring-1 ring-blue-500/30`.
   - Search icon (magnifying glass SVG) on the left.
   - Input: `font-mono text-xs`, placeholder "Find tag...".
   - Searching indicator: "searching..." text with `animate-pulse`, positioned absolutely inside the input.
   - **Enter** key triggers search forward.
8. **Cancel Button** (conditional): Visible only during active search. Red themed (`bg-red-900/60 hover:bg-red-800 text-red-300`), ✕ icon.
9. **Prev/Next Buttons**: Chevron SVG icons (`<` / `>`), `w-7 h-7`.
10. **XPath Display Box**: Fills remaining space (`flex-1`).
    - Container: `font-mono text-sm`, `bg-gray-800/60 border border-gray-700 rounded`, truncated text.
    - **States**:
      - **Default**: Shows `xpath` label + current path (or `/`).
      - **Loading**: Blue spinner + "Locating element..." with `animate-pulse`.
      - **Not Found**: Red ✕ icon + "Not found" text, `border-red-500 ring-1 ring-red-500/30`, auto-dismisses after 2 seconds.
      - **Copied**: `border-green-500 text-green-300`.
    - **Hover tooltip**: Absolute-positioned popover below, full XPath with word-break, "Click to copy" hint.
    - **Click**: Copies XPath to clipboard.

#### Content Area

- **Container**: `flex-1 overflow-auto`, `font-mono text-sm`.
- **Two rendering modes**:

**Three-Panel View** (after a search/navigation):

1. **Context Before** (~2KB): `opacity-60`, padded, syntax-highlighted.
2. **Active Element**: Pretty-printed XML with:
   - `bg-amber-950/40` background.
   - `border-l-4 border-amber-400` left accent bar.
   - `rounded-r-lg`.
   - Auto-scrolls into view (`scrollIntoView({ block: 'center' })`).
   - **Copy button**: Appears on hover (`opacity-0 group-hover:opacity-100`), copies raw element XML.
3. **Context After** (~2KB): `opacity-60`, padded, syntax-highlighted.

**Single Chunk View** (initial state, before any search):

- Displays the first ~5KB of the file with syntax highlighting.

### 4. Syntax Highlighting

Custom CSS classes applied via regex-based tokenizer:

| Token      | Class          | Color                    | Style  |
| ---------- | -------------- | ------------------------ | ------ |
| Tags       | `.xml-tag`     | `#60a5fa` (blue-400)     | Bold   |
| Attributes | `.xml-attr`    | `#c4b5fd` (purple-300)   | Normal |
| Values     | `.xml-val`     | `#86efac` (green-300)    | Normal |
| Comments   | `.xml-comment` | `#6b7280` (gray-500)     | Italic |

The highlighting uses sentinel characters (`\x01`, `\x02`, `\x03`) as intermediate markers to avoid regex conflicts with HTML entities.

## Interaction & Feedback

### Search Feedback

- **Active search**: "searching..." pulse animation inside the search input + cancel button appears.
- **Not found**: Red warning displayed inline in the XPath box for 2 seconds — no modal/alert dialog.
- **Found**: XPath box updates immediately, content scrolls to the matched element.
- **Cancellation**: Cancel button sets an `AtomicBool` flag on the backend, search loop exits on next iteration.

### Copy Feedback

- **XPath copy**: Green border flash on the XPath box for 1.5 seconds.
- **Element copy**: Button text changes to "✅ Copied" for 1.5 seconds.

### Navigation Feedback

- **Loading state**: Blue ring on XPath box + spinner animation while `goToStart` / `goToEnd` operations are in progress.

### Visual Design Principles

- **Compact header**: Icon buttons (`w-7 h-7`) instead of text labels to maximize horizontal space.
- **File name truncation**: Only basename shown, full path accessible via hover tooltip.
- **Progressive disclosure**: Copy buttons and remove buttons only appear on hover.
- **Consistent feedback**: All temporary visual states (copied, not found) auto-dismiss after a short delay.
- **No modal dialogs**: All feedback is inline — no `alert()` calls in the UI flow.