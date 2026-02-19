use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};

static SEARCH_CANCELLED: AtomicBool = AtomicBool::new(false);

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


#[tauri::command]
pub async fn resolve_xpath(path: String, offset: u64, tag_name: String) -> Result<String, String> {
    let parent_path = reconstruct_xpath(&path, offset).map_err(|e| e.to_string())?;
    Ok(format!("{}/{}", parent_path, tag_name))
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
                let approx_end = find_element_end_pos(&mut reader, &mut buf, &name, file_len, 0)?;
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
    search_type: String,
    start_offset: u64,
) -> Result<SearchResult, String> {
    SEARCH_CANCELLED.store(false, Ordering::SeqCst);
    search_node_internal(&app, &path, &query, &search_type, start_offset).map_err(|e| e.to_string())
}

fn search_node_internal(
    app: &AppHandle,
    path: &str,
    query: &str,
    search_type: &str,
    start_offset: u64,
) -> Result<SearchResult> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    
    // Seek to start_offset if > 0
    if start_offset > 0 {
        file.seek(SeekFrom::Start(start_offset))?;
    }

    // Increase buffer size to 1MB for better performance on large files
    let mut reader = quick_xml::Reader::from_reader(BufReader::with_capacity(1024 * 1024, file));
    reader.check_end_names(false);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    // If we sought, we don't know the parents. Push a placeholder or empty?
    // Let's just keep stack empty. The found element will be at top level relative to search.
    if start_offset > 0 {
        // We know the stack is invalid, but we will reconstruct it exactly when needed
    }

    let query_bytes = query.to_lowercase().into_bytes();
    let type_bytes = search_type.to_lowercase().into_bytes();
    
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
                line_number: 0,
            });
        }

        // buffer_position() is relative to where we started reading
        let pos_before = start_offset + reader.buffer_position() as u64;
        
        // Report progress every ~1% or continuously if small
        let current_progress = ((pos_before as f64 / total_len) * 100.0) as u64;
        // Only emit if changed (and since we sought, we are already at start_offset)
        if current_progress > last_progress {
            last_progress = current_progress;
            let _ = app.emit("search-progress", current_progress);
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push(name.clone());

                if element_matches_bytes(e, &query_bytes, &type_bytes) {
                    let approx_start = pos_before as u64;

                    // Find end of element by continuing to parse until matching close tag
                    let approx_end = find_element_end_pos(&mut reader, &mut buf, &name, file_len, start_offset)?; // Needed?

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
                if element_matches_bytes(e, &query_bytes, &type_bytes) {
                    let approx_start = pos_before as u64;
                    let approx_end = start_offset + reader.buffer_position() as u64;

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
        line_number: 0,
    })
}

/// Check if an element matches the query by tag name, guid, id, or name attribute.
/// Uses raw bytes comparison to avoid allocations.
fn element_matches_bytes(e: &BytesStart, query_bytes: &[u8], type_bytes: &[u8]) -> bool {
    // If type is "tag" or "any", check tag name
    if (type_bytes.is_empty() || type_bytes == b"tag" || type_bytes == b"any") && contains_ignore_case(e.name().as_ref(), query_bytes) {
        return true;
    }
    
    // Check specific attributes
    for attr in e.attributes().flatten() {
        let key = attr.key.as_ref();
        
        let mut check_this_attr = false;

        // "any" matches everything in our list
        if type_bytes == b"any" {
            if key_matches(key, b"guid") || 
               key_matches(key, b"id") || 
               key_matches(key, b"name") ||
               key_matches(key, b"eaid") ||
               key_matches(key, b"value") ||
               key_matches(key, b"guidref") {
                check_this_attr = true;
            }
        } else {
            // specific match
            if key_matches(key, type_bytes) {
                check_this_attr = true;
            }
        }
        
        if check_this_attr {
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
    start_offset: u64,
) -> Result<u64> {
    let mut depth = 1u32;
    let initial_pos = reader.buffer_position();
    let scan_limit = 10 * 1024 * 1024; // 10MB limit

    loop {
        buf.clear();
        let current_pos = reader.buffer_position();
        if current_pos - initial_pos > scan_limit {
            // Stop scanning if element is too large
            return Ok(start_offset + current_pos as u64);
        }

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
                        return Ok(start_offset + reader.buffer_position() as u64);
                    }
                }
            }
            Ok(Event::Eof) => return Ok(file_len),
            Err(_) => return Ok(start_offset + reader.buffer_position() as u64),
            _ => (),
        }
    }
}

#[derive(serde::Serialize)]
pub struct SearchResult {
    found: bool,
    xpath: String,
    element_text: String,
    context_before: String,
    context_after: String,
    offset: u64,
    line_number: u64,
}

fn count_lines_up_to(path: &str, offset: u64) -> Result<u64> {
    let file = File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file); // 1MB buffer
    let mut count = 1; // 1-based line number
    let mut total_read = 0;
    
    // Read in chunks
    let mut buffer = [0; 8192];
    loop {
        let remaining = offset - total_read;
        if remaining == 0 {
            break;
        }
        
        let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
        let n = reader.read(&mut buffer[..to_read])?;
        if n == 0 {
            break; // EOF
        }
        
        // Count newlines in this chunk
        count += buffer[..n].iter().filter(|&&b| b == b'\n').count() as u64;
        total_read += n as u64;
    }
    
    Ok(count)
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

    // --- Find exact start: scan backward for '<' ---
    // We start scanning back a bit from approx_start to be safe
    let scan_back = 2048u64;
    let scan_start = if approx_start > scan_back { approx_start - scan_back } else { 0 };
    file.seek(SeekFrom::Start(scan_start))?;
    
    let n = (approx_start - scan_start) as usize;
    let mut scan_buf = vec![0u8; n];
    file.read(&mut scan_buf)?; // Read up to approx_start

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

    // --- Read Element Text ---
    let len = exact_end - exact_start;
    file.seek(SeekFrom::Start(exact_start))?;
    let mut element_buf = vec![0u8; len as usize];
    file.read(&mut element_buf)?;
    let element_text = String::from_utf8_lossy(&element_buf).to_string();

    // --- Read Context Before ---
    let context_len = 2000u64;
    let context_start = if exact_start > context_len { exact_start - context_len } else { 0 };
    file.seek(SeekFrom::Start(context_start))?;
    let context_read_len = (exact_start - context_start) as usize;
    let mut context_before_buf = vec![0u8; context_read_len];
    file.read(&mut context_before_buf)?;
    let context_before = String::from_utf8_lossy(&context_before_buf).to_string();

    // --- Read Context After ---
    file.seek(SeekFrom::Start(exact_end))?;
    let after_len = context_len.min(file_len - exact_end) as usize;
    let mut context_after_buf = vec![0u8; after_len];
    file.read(&mut context_after_buf)?;
    let context_after = String::from_utf8_lossy(&context_after_buf).to_string();

    // --- Count Lines ---
    let line_number = count_lines_up_to(path, exact_start).unwrap_or(0);

    Ok(SearchResult {
        found: true,
        xpath: xpath.to_string(),
        element_text,
        context_before,
        context_after,
        offset: exact_start,
        line_number,
    })
}

fn reconstruct_xpath(path: &str, target_offset: u64) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = quick_xml::Reader::from_reader(BufReader::with_capacity(1024 * 1024, file));
    reader.check_end_names(false);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();

    loop {
        // Must check position BEFORE reading event
        // Note: buffer_position is relative to start of file (since we started from 0)
        let pos = reader.buffer_position() as u64;
        if pos >= target_offset {
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push(name);
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break, // Ignore errors, just do best effor
            _ => {}
        }
        buf.clear();
    }
    Ok(format!("/{}", stack.join("/")))
}

#[tauri::command]
pub async fn find_parent(path: String, child_offset: u64, ancestor_depth: u32) -> Result<SearchResult, String> {
    find_parent_internal(&path, child_offset, ancestor_depth).map_err(|e| e.to_string())
}

fn find_parent_internal(path: &str, child_offset: u64, ancestor_depth: u32) -> Result<SearchResult> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut reader = quick_xml::Reader::from_reader(BufReader::with_capacity(1024 * 1024, file));
    reader.check_end_names(false);

    let mut buf = Vec::new();
    // Stack of (tag_name, byte_position_before_start_event)
    let mut stack: Vec<(String, u64)> = Vec::new();

    loop {
        let pos_before = reader.buffer_position() as u64;
        if pos_before >= child_offset {
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push((name, pos_before));
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // The stack now contains ancestors of the element at child_offset.
    let depth = ancestor_depth as usize;
    if depth >= stack.len() {
        return Err(anyhow::anyhow!("Ancestor depth {} is out of range (stack has {} entries)", depth, stack.len()));
    }

    let (ancestor_name, ancestor_start) = stack[depth].clone();

    // Build the XPath up to and including the target ancestor
    let xpath = format!("/{}", stack[..=depth].iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join("/"));

    // Find the end of the ancestor element by seeking to its start and parsing
    let mut file3 = File::open(path)?;
    file3.seek(SeekFrom::Start(ancestor_start))?;
    let mut reader3 = quick_xml::Reader::from_reader(BufReader::with_capacity(1024 * 1024, file3));
    reader3.check_end_names(false);

    let mut buf3 = Vec::new();
    // Read the first Start event (the ancestor itself)
    loop {
        match reader3.read_event_into(&mut buf3) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == ancestor_name {
                    break;
                }
            }
            Ok(Event::Eof) => return Err(anyhow::anyhow!("Unexpected EOF while seeking ancestor start")),
            Err(e) => return Err(anyhow::anyhow!("Parse error: {:?}", e)),
            _ => {}
        }
        buf3.clear();
    }

    // Now find the matching end tag
    let approx_end = find_element_end_pos(&mut reader3, &mut buf3, &ancestor_name, file_len, ancestor_start)?;

    extract_and_build_result(path, file_len, ancestor_start, approx_end, &xpath)
}
