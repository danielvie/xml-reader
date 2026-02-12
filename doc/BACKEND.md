# Backend Documentation - XML Reader

This document provides a detailed explanation of the Rust functions and Tauri commands implemented in the `src-tauri/src/xml_ops.rs` module.

## 🏗️ Architecture Overview

The backend is designed for high performance and low memory consumption when handling large XML files. Instead of loading the entire file into memory, it uses **streaming** and **random access** (seeking) to read and parse only the necessary chunks of data.

A global `AtomicBool` flag (`SEARCH_CANCELLED`) enables cooperative cancellation of long-running search operations from the frontend.

---

## 🛰️ Tauri Commands (Public API)

These functions are exposed to the frontend via the `invoke` system and registered in `src-tauri/src/lib.rs`.

### `generate_large_xml(path, size_mb, depth)`

Generates a valid, large XML file for testing purposes.

- **Parameters**:
    - `path`: Absolute filesystem path for the output file.
    - `size_mb`: Target file size in Megabytes (up to 2048).
    - `depth`: Maximum nesting depth of the XML tree (1–10).
- **How it works**: Uses `BufWriter` and `quick-xml` to efficiently stream elements to disk. Creates parent directories if they don't exist. Each element receives `guid`, `id`, and `name` attributes for searchability.
- **Generated structure**: Elements are named `item_{level}_{id}` and nested recursively up to `max_depth`. Leaf elements contain text content with their node name and GUID.

### `open_file(path)`

Prepares a file for reading.

- **Returns**: The total size of the file in bytes (`u64`).
- **Use case**: Initializes the frontend viewer state, enabling offset-based navigation and scroll calculations.

### `read_chunk(path, offset, size)`

Reads a specific segment of the file as raw text.

- **Parameters**:
    - `path`: File path.
    - `offset`: Starting byte position (`u64`).
    - `size`: Number of bytes to read (`u32`).
- **Returns**: A UTF-8 string of the requested chunk (lossy conversion).
- **Efficiency**: Only the requested bytes are read from disk using `Seek::SeekFrom::Start`.

### `search_node(path, query, start_offset)`

Advanced search that scans the file for matching XML elements.

- **Parameters**:
    - `path`: File path.
    - `query`: The string to search for (case-insensitive, matches tag names, `id`, `guid`, or `name` attributes).
    - `start_offset`: Byte position to start searching from (enables "Find Next" by passing `last_match_offset + 1`).
- **Returns**: A `SearchResult` object containing the match status, XPath, element content, surrounding context, and byte offset.
- **Cancellation**: Resets the `SEARCH_CANCELLED` flag to `false` before starting. The search loop checks this flag on every iteration and returns early with `found: false` if cancellation is requested.

### `cancel_search()`

Cancels a running search operation.

- **How it works**: Sets the global `SEARCH_CANCELLED` `AtomicBool` to `true` using `SeqCst` ordering. The search loop in `search_node_internal` checks this flag on each iteration and exits early if set.
- **Returns**: `Ok(())` — always succeeds.

### `get_first_child(path)`

Navigates to the first child element of the root.

- **How it works**: Linear scan from the beginning of the file using `quick-xml::Reader`. Skips the `<root>` element and returns the first `Start` or `Empty` event encountered.
- **Returns**: A `SearchResult` with XPath set to `/root (first)`.

### `get_last_child(path)`

Navigates to the last child element of the root.

- **How it works**: **Reverse scan** from the end of the file in 64KB chunks. Parses tags directly from raw bytes (without a full XML reader) using a depth counter to find the last complete top-level element. Skips `<root>` tags.
- **Efficiency**: Critical for navigating multi-gigabyte files without scanning from the beginning. Uses a single file handle with manual seeking.
- **Returns**: A `SearchResult` with XPath set to `/root (last)`.

---

## ⚙️ Internal Logic & Helpers

### `generate_xml_internal(path, size_mb, depth)`

Core logic for `generate_large_xml`. Opens a `BufWriter`, writes a `<root>` wrapper, and loops `write_nested_element` until the target byte size is reached.

### `write_nested_element(writer, item_count, current_level, max_depth)`

Recursive function used by the generator. Creates an element with:
- **Tag name**: `item_{level}_{id}` (e.g., `item_1_0`, `item_2_1`).
- **Attributes**: `guid` (pseudo-UUID), `id` (counter), `name` (human-readable like `Alpha_L1`).
- **Children**: If `current_level < max_depth`, recurses to create a child element. Otherwise, writes text content.
- **Returns**: Estimated bytes written (used by the size-check loop).

### `pseudo_uuid(n)`

Generates deterministic, unique-looking UUIDs from a counter using wrapping multiplication and addition. Produces consistent results for the same input, making generated samples reproducible.

### `item_name(level, n)`

Generates human-readable names from a counter by cycling through a list of Greek-letter names (`Alpha`, `Beta`, `Gamma`, etc.) with a level suffix (e.g., `Delta_L3`).

### `search_node_internal(path, query, start_offset)`

Core search logic. Uses `quick-xml::Reader` to stream through the file:
1. Maintains a `stack: Vec<String>` to track the current XPath.
2. On each `Start` or `Empty` event, calls `element_matches` to check the query.
3. If a match is found at or after `start_offset`, determines the element boundaries and delegates to `extract_and_build_result`.
4. Checks `SEARCH_CANCELLED` on every loop iteration for cooperative cancellation.

### `element_matches(e, tag_name, query_lower)`

Case-insensitive matching logic. Returns `true` if `query_lower` is contained in any of:
- The **tag name**.
- The **`id`** attribute value.
- The **`guid`** attribute value.
- The **`name`** attribute value.

### `find_element_end_pos(reader, buf, tag_name, file_len)`

Handles complex nested structures after a `Start` event match. Tracks a depth counter and continues parsing until the matching `End` event is found (depth returns to 0). Returns the byte position after the closing tag.

### `extract_and_build_result(path, file_len, approx_start, approx_end, xpath)`

A critical helper for all navigation and search operations. Given approximate byte positions from the XML parser:
1. **Scans backward** (up to 128 bytes) to find the exact opening `<` character.
2. **Scans forward** (up to 128 bytes) to find the exact closing `>` character.
3. **Reads the element text** between the exact boundaries.
4. **Captures ~2KB of context** before and after the element for the viewer UI.
5. Returns a complete `SearchResult` with `found: true`.

### `get_first_child_internal(path)` / `get_last_child_internal(path)`

Internal implementations for the navigation commands. See the Tauri command descriptions above for algorithmic details.

### Tag Classification Helpers (for reverse scanning)

#### `TagKind` enum

Classifies a tag as `Open`, `Close`, or `Empty`.

#### `classify_tag(slice)`

Given a complete tag byte slice `<…>`, returns `(name, kind, byte_len)`. Returns `None` for processing instructions (`<?…?>`), comments, and CDATA sections (`<!…>`).

#### `extract_tag_name_from_bytes(bytes)`

Extracts the tag name from a byte slice by scanning until a delimiter (space, tab, newline, `>`, or `/`).

---

## 📦 Data Structures

### `SearchResult` (JSON Serializable)

```rust
#[derive(serde::Serialize)]
pub struct SearchResult {
    found: bool,          // Whether a match was found
    xpath: String,        // Full XPath of the matched element (e.g., "/root/item_1_0/item_2_1")
    element_text: String, // Raw XML text of the matched element
    context_before: String, // ~2KB of file content before the element
    context_after: String,  // ~2KB of file content after the element
    offset: u64,          // Byte offset of the element in the file
}
```

This structure provides the frontend with everything needed to render a match with surrounding context and enable "Find Next" / "Find Previous" navigation.

---

## 🔗 Command Registration

All commands are registered in `src-tauri/src/lib.rs`:

```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        xml_ops::generate_large_xml,
        xml_ops::open_file,
        xml_ops::read_chunk,
        xml_ops::search_node,
        xml_ops::cancel_search,
        xml_ops::get_first_child,
        xml_ops::get_last_child
    ])
```

---

## 📚 Dependencies

| Crate       | Version | Purpose                                    |
| ----------- | ------- | ------------------------------------------ |
| `tauri`     | 2.x     | Application framework and IPC              |
| `quick-xml` | 0.31    | Streaming XML parsing and writing          |
| `anyhow`    | 1.0     | Ergonomic error handling                   |
| `serde`     | 1.x     | Serialization of `SearchResult` to JSON    |
| `serde_json`| 1.x     | JSON support                               |