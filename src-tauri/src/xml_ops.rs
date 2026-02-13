use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use quick_xml::writer::Writer;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static SEARCH_CANCELLED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn generate_large_xml(path: String, size_mb: u32, depth: u32) -> Result<(), String> {
    generate_xml_internal(&path, size_mb, depth).map_err(|e| e.to_string())
}

fn generate_xml_internal(path: &str, size_mb: u32, depth: u32) -> Result<()> {
    let target_size = (size_mb as u64) * 1024 * 1024;
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = Writer::new(BufWriter::new(file));

    let root = BytesStart::new("root");
    writer.write_event(Event::Start(root.clone()))?;

    let mut current_size = 0u64;
    let mut item_count = 0u64;
    let max_depth = depth.max(1);

    while current_size < target_size {
        let written = write_nested_element(&mut writer, &mut item_count, 1, max_depth)?;
        current_size += written;
    }

    writer.write_event(Event::End(root.to_end()))?;
    writer.into_inner().flush()?;

    Ok(())
}

/// Generate a pseudo-UUID from a counter for deterministic but unique-looking IDs.
fn pseudo_uuid(n: u64) -> String {
    let h = n
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let a = (h >> 32) as u32;
    let b = (h >> 16) as u16;
    let c = (h & 0xFFFF) as u16;
    let d = n as u16;
    let e = (n >> 16) as u32 ^ 0xDEAD;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:08x}{:04x}",
        a,
        b,
        c,
        d,
        e,
        (n & 0xFFFF) as u16
    )
}

/// Generate a human-readable name from a counter.
fn item_name(level: u32, n: u64) -> String {
    let names = [
        "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota", "Kappa",
    ];
    let base = names[(n % names.len() as u64) as usize];
    format!("{}_L{}", base, level)
}

fn write_nested_element(
    writer: &mut Writer<BufWriter<File>>,
    item_count: &mut u64,
    current_level: u32,
    max_depth: u32,
) -> Result<u64> {
    let id = *item_count;
    let node_name = format!("item_{}_{}", current_level, id);
    *item_count += 1;

    let guid = pseudo_uuid(id);
    let name = item_name(current_level, id);

    let mut node = BytesStart::new(&node_name);
    node.push_attribute(("guid", guid.as_str()));
    node.push_attribute(("id", id.to_string().as_str()));
    node.push_attribute(("name", name.as_str()));
    writer.write_event(Event::Start(node.clone()))?;

    // Rough byte estimate: open tag + attrs + close tag
    let mut bytes_written = node_name.len() as u64 * 2 + guid.len() as u64 + name.len() as u64 + 60;

    if current_level >= max_depth {
        let content = format!("Content for {} (guid={}).", node_name, guid);
        writer.write_event(Event::Text(quick_xml::events::BytesText::new(&content)))?;
        bytes_written += content.len() as u64;
    } else {
        let child_bytes = write_nested_element(writer, item_count, current_level + 1, max_depth)?;
        bytes_written += child_bytes;
    }

    writer.write_event(Event::End(node.to_end()))?;

    Ok(bytes_written)
}

#[tauri::command]
pub async fn open_file(path: String) -> Result<u64, String> {
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    Ok(len)
}

#[tauri::command]
pub async fn read_chunk(path: String, offset: u64, size: u32) -> Result<String, String> {
    read_chunk_internal(&path, offset, size).map_err(|e| e.to_string())
}

fn read_chunk_internal(path: &str, offset: u64, size: u32) -> Result<String> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;

    let mut buffer = vec![0; size as usize];
    let n = file.read(&mut buffer)?;

    let s = String::from_utf8_lossy(&buffer[..n]).to_string();
    Ok(s)
}

#[derive(serde::Serialize)]
pub struct SearchResult {
    found: bool,
    xpath: String,
    element_text: String,
    context_before: String,
    context_after: String,
    offset: u64,
}

#[tauri::command]
pub async fn get_first_child(path: String) -> Result<SearchResult, String> {
    get_first_child_internal(&path).map_err(|e| e.to_string())
}

fn get_first_child_internal(path: &str) -> Result<SearchResult> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut reader = quick_xml::Reader::from_reader(BufReader::new(file));

    let mut buf = Vec::new();
    let mut root_found = false;
    let mut root_name = String::new();

    // Loop until we find the first child element (the one after <root>)
    loop {
        let pos_before = reader.buffer_position();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if !root_found {
                    root_found = true;
                    root_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    continue;
                }

                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                // Found first child!
                let approx_start = pos_before as u64;
                let approx_end = find_element_end_pos(&mut reader, &mut buf, &name, file_len)?;
                let xpath = format!("/{}/{} (first)", root_name, name);

                return extract_and_build_result(path, file_len, approx_start, approx_end, &xpath);
            }
            Ok(Event::Empty(ref e)) => {
                if !root_found {
                    // Root is self-closing -> <Root />. No children.
                    return Err(anyhow::anyhow!("Root element is empty (no children)").into());
                }

                // First child is empty self-closing tag
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let approx_start = pos_before as u64;
                let approx_end = reader.buffer_position() as u64;
                let xpath = format!("/{}/{} (first)", root_name, name);
                return extract_and_build_result(path, file_len, approx_start, approx_end, &xpath);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error: {:?}", e)),
            _ => (),
        }
        buf.clear();
    }

    Err(anyhow::anyhow!("No child elements found").into())
}

#[tauri::command]
pub async fn get_last_child(path: String) -> Result<SearchResult, String> {
    get_last_child_internal(&path).map_err(|e| e.to_string())
}

fn get_last_child_internal(path: &str) -> Result<SearchResult> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len == 0 {
        return Err(anyhow::anyhow!("File is empty"));
    }

    let mut depth: i32 = 0;
    let mut last_tag_end: Option<u64> = None;
    let mut root_name = String::new();

    // Scan backwards from the end in chunks.
    // Uses a SINGLE file handle and parses tags directly from the buffer
    // instead of opening a new File + quick_xml::Reader per '<'.
    let chunk_size: usize = 64 * 1024;
    let mut current_pos = len;
    let mut buf = vec![0u8; chunk_size];
    // Small buffer for tags that span a chunk boundary (very rare)
    let mut tag_buf = vec![0u8; 1024];

    while current_pos > 0 {
        let read_size = std::cmp::min(current_pos, chunk_size as u64) as usize;
        current_pos -= read_size as u64;

        file.seek(SeekFrom::Start(current_pos))?;
        file.read_exact(&mut buf[..read_size])?;

        for i in (0..read_size).rev() {
            if buf[i] != b'<' {
                continue;
            }

            let abs_start = current_pos + i as u64;
            let remaining = &buf[i..read_size];

            // Resolve the full tag bytes up to the closing '>'.
            let parsed = if let Some(gt) = remaining.iter().position(|&b| b == b'>') {
                classify_tag(&remaining[..gt + 1])
            } else {
                // Tag spans chunk boundary — one small forward read (very rare)
                let to_read = std::cmp::min(1024u64, len - abs_start) as usize;
                file.seek(SeekFrom::Start(abs_start))?;
                let n = file.read(&mut tag_buf[..to_read])?;
                if let Some(gt) = tag_buf[..n].iter().position(|&b| b == b'>') {
                    classify_tag(&tag_buf[..gt + 1])
                } else {
                    None
                }
            };

            let (tag_name, tag_kind, tag_len) = match parsed {
                Some(v) => v,
                None => continue, // PI, comment, CDATA, or malformed — skip
            };

            match tag_kind {
                TagKind::Close => {
                    depth += 1;

                    if depth == 1 {
                        root_name = tag_name;
                    }

                    // We want the child at depth 1 (child of root).
                    // When we see </Child>, depth goes 1->2.
                    if depth == 2 {
                        last_tag_end = Some(abs_start + tag_len as u64);
                    }
                }
                TagKind::Open => {
                    depth -= 1;
                    // Opening tag for child at depth 1. depth 2->1.
                    if depth == 1 {
                        if let Some(end) = last_tag_end {
                            let xpath = format!("/{}/{} (last)", root_name, tag_name);
                            return extract_and_build_result(path, len, abs_start, end, &xpath);
                        }
                    }
                    // If we hit depth 0 (<Root>), we are done searching children.
                    if depth == 0 {
                        return Err(anyhow::anyhow!("No last child found (root exited)"));
                    }
                }
                TagKind::Empty => {
                    // <Child /> at depth 1.
                    if depth == 1 {
                        let abs_end = abs_start + tag_len as u64;
                        let xpath = format!("/{}/{} (last)", root_name, tag_name);
                        return extract_and_build_result(path, len, abs_start, abs_end, &xpath);
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!("Last child not found"))
}

// ── Lightweight tag classification for the backwards scanner ──────────────

#[derive(Debug, PartialEq)]
enum TagKind {
    Open,
    Close,
    Empty,
}

/// Given a complete tag slice `<…>`, return `(name, kind, byte_len)`.
/// Returns `None` for processing instructions (`<?`), comments/CDATA (`<!`).
fn classify_tag(slice: &[u8]) -> Option<(String, TagKind, usize)> {
    if slice.len() < 3 {
        return None;
    }
    if slice[1] == b'?' || slice[1] == b'!' {
        return None;
    }

    let len = slice.len();
    if slice[1] == b'/' {
        // Closing tag  </name …>
        let name = extract_tag_name_from_bytes(&slice[2..]);
        Some((name, TagKind::Close, len))
    } else if len >= 2 && slice[len - 2] == b'/' {
        // Self-closing  <name … />
        let name = extract_tag_name_from_bytes(&slice[1..]);
        Some((name, TagKind::Empty, len))
    } else {
        // Opening tag  <name …>
        let name = extract_tag_name_from_bytes(&slice[1..]);
        Some((name, TagKind::Open, len))
    }
}

/// Scan bytes until a delimiter and return the tag name.
fn extract_tag_name_from_bytes(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/'))
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

#[tauri::command]
pub async fn cancel_search() -> Result<(), String> {
    SEARCH_CANCELLED.store(true, Ordering::SeqCst);
    Ok(())
}

use tauri::{AppHandle, Emitter};

#[tauri::command]
pub async fn search_node(
    app: AppHandle,
    path: String,
    query: String,
    start_offset: u64,
) -> Result<SearchResult, String> {
    SEARCH_CANCELLED.store(false, Ordering::SeqCst);
    search_node_internal(&app, &path, &query, start_offset).map_err(|e| e.to_string())
}

fn search_node_internal(
    app: &AppHandle,
    path: &str,
    query: &str,
    start_offset: u64,
) -> Result<SearchResult> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    // Increase buffer size to 1MB for better performance on large files
    let mut reader = quick_xml::Reader::from_reader(BufReader::with_capacity(1024 * 1024, file));

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let query_bytes = query.to_lowercase().into_bytes();

    let mut last_progress = 0u64;
    let total_len = file_len as f64;

    loop {
        if SEARCH_CANCELLED.load(Ordering::SeqCst) {
            return Ok(SearchResult {
                found: false,
                xpath: String::new(),
                element_text: String::new(),
                context_before: String::new(),
                context_after: String::new(),
                offset: 0,
            });
        }

        let pos_before = reader.buffer_position();

        // Report progress every ~1% or continuously if small
        let current_progress = ((pos_before as f64 / total_len) * 100.0) as u64;
        if current_progress > last_progress {
            last_progress = current_progress;
            let _ = app.emit("search-progress", current_progress);
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push(name.clone());

                if pos_before as u64 >= start_offset && element_matches_bytes(e, &query_bytes) {
                    let approx_start = pos_before as u64;

                    // Find end of element by continuing to parse until matching close tag
                    let approx_end = find_element_end_pos(&mut reader, &mut buf, &name, file_len)?;

                    let xpath = format!("/{}", stack.join("/"));
                    stack.pop();

                    // Emit 100% progress on find
                    let _ = app.emit("search-progress", 100u64);

                    // Extract exact text from the file
                    return extract_and_build_result(
                        path,
                        file_len,
                        approx_start,
                        approx_end,
                        &xpath,
                    );
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if pos_before as u64 >= start_offset && element_matches_bytes(e, &query_bytes) {
                    let approx_start = pos_before as u64;
                    let approx_end = reader.buffer_position() as u64;

                    let mut current_path = stack.clone();
                    current_path.push(name.clone());
                    let xpath = format!("/{}", current_path.join("/"));

                    // Emit 100% progress on find
                    let _ = app.emit("search-progress", 100u64);

                    return extract_and_build_result(
                        path,
                        file_len,
                        approx_start,
                        approx_end,
                        &xpath,
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ))
            }
            _ => (),
        }
        buf.clear();
    }

    // Emit 100% progress on end
    let _ = app.emit("search-progress", 100u64);

    Ok(SearchResult {
        found: false,
        xpath: String::new(),
        element_text: String::new(),
        context_before: String::new(),
        context_after: String::new(),
        offset: 0,
    })
}

/// Check if an element matches the query by tag name, guid, id, or name attribute.
/// Uses raw bytes comparison to avoid allocations.
fn element_matches_bytes(e: &BytesStart, query_bytes: &[u8]) -> bool {
    // Check tag name
    if contains_ignore_case(e.name().as_ref(), query_bytes) {
        return true;
    }

    // Check specific attributes
    for attr in e.attributes().flatten() {
        // key comparison (case-insensitive not strictly needed for standard attributes but good for robustness)
        // We only care about specific keys. Check them directly.
        let key = attr.key.as_ref();
        if key_matches(key, b"guid") || key_matches(key, b"id") || key_matches(key, b"name") {
            if contains_ignore_case(&attr.value, query_bytes) {
                return true;
            }
        }
    }
    false
}

#[inline(always)]
fn key_matches(key: &[u8], target: &[u8]) -> bool {
    if key.len() != target.len() {
        return false;
    }
    for (b1, b2) in key.iter().zip(target.iter()) {
        if !b1.eq_ignore_ascii_case(b2) {
            return false;
        }
    }
    true
}

fn contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    // Windows over haystack
    for window in haystack.windows(needle.len()) {
        if iter_eq_ignore_case(window, needle) {
            return true;
        }
    }
    false
}

#[inline(always)]
fn iter_eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.eq_ignore_ascii_case(y))
}

/// After consuming a Start event, continue parsing until the matching End event.
/// Returns the approximate byte position after the closing tag.
fn find_element_end_pos(
    reader: &mut quick_xml::Reader<BufReader<File>>,
    buf: &mut Vec<u8>,
    tag_name: &str,
    file_len: u64,
) -> Result<u64> {
    let mut depth = 1u32;
    loop {
        buf.clear();
        match reader.read_event_into(buf) {
            Ok(Event::Start(ref e)) => {
                let qname = e.name();
                let name = String::from_utf8_lossy(qname.as_ref());
                if name == tag_name {
                    depth += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let name = String::from_utf8_lossy(qname.as_ref());
                if name == tag_name {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(reader.buffer_position() as u64);
                    }
                }
            }
            Ok(Event::Eof) => return Ok(file_len),
            Err(_) => return Ok(reader.buffer_position() as u64),
            _ => (),
        }
    }
}

/// Given approximate start/end positions from quick-xml, find the exact element
/// boundaries in the file and extract the text + surrounding context.
fn extract_and_build_result(
    path: &str,
    file_len: u64,
    approx_start: u64,
    approx_end: u64,
    xpath: &str,
) -> Result<SearchResult> {
    let mut file = File::open(path)?;

    // --- Find exact start: scan backwards for '<' ---
    // --- Find exact start: scan backwards for '<' ---
    // If approx_start points to whitespace/content after a tag, we need to find the START of the current tag.
    let search_back = 128u64;
    let scan_start = approx_start.saturating_sub(search_back);
    file.seek(SeekFrom::Start(scan_start))?;
    let scan_len = (approx_start - scan_start + 1) as usize;
    let mut scan_buf = vec![0u8; scan_len];
    let n = file.read(&mut scan_buf)?;

    // We want the LAST '<' in the buffer that is NOT part of a closing tag like '</...>'.
    // Actually, quick-xml event position is usually at the start of the event.
    // If it points to '<', we are good.
    // If it points to ' ' (whitespace), we want the NEXT '<', not the previous one.
    // BUT pos_before is captured *before* the event.

    // Check if the byte at the very end (approx_start) is '<'.
    let mut exact_start = approx_start;

    // Safety check: verify if we are actually at '<'
    if n > 0 {
        let last_byte_idx = n - 1;
        if scan_buf[last_byte_idx] == b'<' {
            // Perfect, we are at the start.
            exact_start = scan_start + last_byte_idx as u64;
        } else {
            // We are NOT at '<'. This happens if pos_before included leading whitespace.
            // We should scan FORWARD to find the '<', because the event we just read STARTED here (or after).
            // But waiting... we read the event successfully. The event content starts shortly after pos_before.
            // So we should search *forward* from approx_start, not backward.

            // Let's read forward a bit.
            let fwd_search_len = 256;
            file.seek(SeekFrom::Start(approx_start))?;
            let mut buf_fwd = vec![0u8; fwd_search_len];
            let n_fwd = file.read(&mut buf_fwd)?;

            for i in 0..n_fwd {
                if buf_fwd[i] == b'<' {
                    exact_start = approx_start + i as u64;
                    break;
                }
            }
        }
    }

    // --- Find exact end: scan forward for '>' ---
    let scan_fwd = 128u64;
    let fwd_start = if approx_end > 0 { approx_end - 1 } else { 0 };
    file.seek(SeekFrom::Start(fwd_start))?;
    let fwd_len = scan_fwd.min(file_len - fwd_start) as usize;
    let mut fwd_buf = vec![0u8; fwd_len];
    let n2 = file.read(&mut fwd_buf)?;

    let mut exact_end = approx_end;
    for i in 0..n2 {
        if fwd_buf[i] == b'>' {
            exact_end = fwd_start + i as u64 + 1; // +1 to include the '>'
            break;
        }
    }

    // --- Read element text ---
    let elem_len = (exact_end - exact_start) as usize;
    file.seek(SeekFrom::Start(exact_start))?;
    let mut elem_buf = vec![0u8; elem_len];
    file.read_exact(&mut elem_buf)?;
    let element_text = String::from_utf8_lossy(&elem_buf).to_string();

    // --- Read context before (up to 2KB) ---
    let ctx_size = 10000u64;
    let before_start = exact_start.saturating_sub(ctx_size);
    let before_len = (exact_start - before_start) as usize;
    let context_before = if before_len > 0 {
        file.seek(SeekFrom::Start(before_start))?;
        let mut bb = vec![0u8; before_len];
        let nb = file.read(&mut bb)?;
        String::from_utf8_lossy(&bb[..nb]).to_string()
    } else {
        String::new()
    };

    // --- Read context after (up to 2KB) ---
    let after_len = ctx_size.min(file_len - exact_end) as usize;
    let context_after = if after_len > 0 {
        file.seek(SeekFrom::Start(exact_end))?;
        let mut ab = vec![0u8; after_len];
        let na = file.read(&mut ab)?;
        String::from_utf8_lossy(&ab[..na]).to_string()
    } else {
        String::new()
    };

    Ok(SearchResult {
        found: true,
        xpath: xpath.to_string(),
        element_text,
        context_before,
        context_after,
        offset: exact_start,
    })
}
