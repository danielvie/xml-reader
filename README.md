# XML Reader

A high-performance Desktop XML Reader built with **Tauri**, **Rust**, and **Svelte 5**. Designed specifically to handle massive XML files (up to 2GB+) with a modern, fluid interface and minimal memory footprint.

## 🚀 The Goal

Existing XML viewers often struggle or crash when handling files larger than a few hundred megabytes. This project aims to provide a professional-grade tool that remains performant and responsive even when exploring multi-gigabyte XML structures.

## ✨ Key Features

- **Blazing Fast Performance**: Uses Rust streaming (Seek-based BufReader) to read only what's needed for the current view.
- **Massive File Support**: Seamlessly navigate files up to 2GB and beyond without loading the entire content into RAM.
- **Smart Search & Navigation**:
  - Search by Tag Name, `id`, `guid`, or `name` attribute.
  - Jump to Start/End of large trees instantly.
  - Previous/Next match navigation.
  - **Cancellable search**: abort long-running searches with a single click.
- **Inline Search Feedback**: "Not found" warnings appear directly in the XPath bar (no disruptive alert dialogs).
- **Real-time XPath**: Displays the exact XPath of the currently active element with single-click copy-to-clipboard functionality.
- **Modern Syntax Highlighting**: A clean, dark-themed XML viewer with optimized syntax coloring for readability.
- **Custom XML Generator**: Built-in utility to generate large, valid XML sample files with customizable depth and size for testing (presets from 10MB to 2GB).
- **Drag-and-Drop Workflow**: Intuitive landing page with file drop-zone and recent files history (last 10 files persisted).

## 🛠️ Tech Stack

- **Backend**: [Rust](https://www.rust-lang.org/) with [Tauri](https://tauri.app/) (utilizing `quick-xml` for low-level parsing).
- **Frontend**: [Svelte 5](https://svelte.dev/) for reactive state management and UI components.
- **Styling**: [Tailwind CSS](https://tailwindcss.com/) with a curated Dark Mode (Glassmorphism effect).

## 📐 Architecture

- **Streaming Architecture**: The frontend requests specific byte chunks from the Rust backend, ensuring UI responsiveness regardless of file size.
- **State Management**: Uses Svelte 5 runes (`$state`, `$derived`) for efficient, granular UI updates.
- **Cooperative Cancellation**: A global `AtomicBool` flag allows the frontend to cancel in-progress search operations without killing threads.
- **Deterministic Samples**: The XML generator creates unique but predictable IDs and structures for consistent testing.

## 🚀 Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/) (LTS recommended)

### Development

```bash
# Install dependencies
npm install

# Run the app in development mode
npm run tauri dev
```

### Build

```bash
# Build the production bundle
npm run tauri build
```

---

## 📄 Documentation

For more detailed technical specs and design principles, see:
- [SPEC.md](./doc/SPEC.md) - Requirements and functionality specification.
- [LAYOUT.md](./doc/LAYOUT.md) - UI/UX standards, color palettes, and component layouts.
- [BACKEND.md](./doc/BACKEND.md) - Detailed documentation of Rust backend logic and commands.
