# Architecture - XML Reader

This document describes the high-level architecture of the application, including component relationships, data flow, and key design decisions.

---

## Table of Contents

- [System Overview](#system-overview)
- [Project Structure](#project-structure)
- [Component Tree](#component-tree)
- [IPC Command Map](#ipc-command-map)
- [State Management](#state-management)
- [Search Lifecycle](#search-lifecycle)
- [File I/O Strategy](#file-io-strategy)
- [Navigation Flow](#navigation-flow)
- [Backend Module Structure](#backend-module-structure)

---

## System Overview

The application follows Tauri's split-process architecture: a **Rust backend** handles all file I/O and XML parsing, while a **Svelte 5 frontend** manages the UI and user interactions. Communication between the two happens through Tauri's IPC `invoke` system.

```mermaid
graph TB
    subgraph Desktop["Desktop Application (Tauri)"]
        subgraph Frontend["Frontend Process (WebView)"]
            SvelteApp["Svelte 5 App"]
            AppState["AppState (Reactive Store)"]
            SvelteApp --> AppState
        end

        subgraph Backend["Backend Process (Rust)"]
            Commands["Tauri Commands"]
            XmlOps["xml_ops module"]
            QuickXml["quick-xml parser"]
            Commands --> XmlOps
            XmlOps --> QuickXml
        end

        AppState -- "invoke()" --> Commands
        Commands -- "JSON Result" --> AppState
    end

    FS["File System\n(.xml files)"]
    Backend --> FS
    FS --> Backend

    LocalStorage["localStorage\n(recent files)"]
    Frontend --> LocalStorage
```

---

## Project Structure

```mermaid
graph LR
    subgraph Root["xml-reader/"]
        README["README.md"]
        PackageJson["package.json"]
        ViteConfig["vite.config.js"]
        SvelteConfig["svelte.config.js"]

        subgraph Doc["doc/"]
            SPEC["SPEC.md"]
            DESIGN["DESIGN.md"]
            DOC_BACK["DOC_BACK.md"]
            ARCH["ARCHITECTURE.md"]
        end

        subgraph Src["src/"]
            AppHtml["app.html"]
            AppCss["app.css"]

            subgraph Routes["routes/"]
                Layout["+layout.svelte\n+layout.ts"]
                Page["+page.svelte"]
            end

            subgraph Lib["lib/"]
                State["state.svelte.ts"]
                subgraph Components["components/"]
                    DropZone["DropZone.svelte"]
                    Header["Header.svelte"]
                    Viewer["Viewer.svelte"]
                    subgraph Icons["icons/"]
                        IconHome["IconHome"]
                        IconFirst["IconElementFirst"]
                        IconSecond["IconElementSecond"]
                    end
                end
            end
        end

        subgraph SrcTauri["src-tauri/"]
            LibRs["lib.rs"]
            XmlOpsRs["xml_ops.rs"]
            CargoToml["Cargo.toml"]
            TauriConf["tauri.conf.json"]
            subgraph TauriIcons["icons/"]
                ICO["icon.ico"]
                ICNS["icon.icns"]
                PNGs["*.png"]
            end
        end

        subgraph Static["static/"]
            Favicon["favicon.png"]
        end
    end

    style Root fill:#1e1e2e,stroke:#6c7086,color:#cdd6f4
    style Src fill:#1e1e2e,stroke:#89b4fa,color:#cdd6f4
    style SrcTauri fill:#1e1e2e,stroke:#fab387,color:#cdd6f4
    style Lib fill:#1e1e2e,stroke:#a6e3a1,color:#cdd6f4
```

### Directory Responsibilities

| Directory          | Purpose                                            |
| ------------------ | -------------------------------------------------- |
| `src/routes/`      | SvelteKit pages and layout (SPA mode, SSR disabled)|
| `src/lib/`         | Shared state and reusable components               |
| `src/lib/components/` | UI components (DropZone, Header, Viewer)        |
| `src-tauri/src/`   | Rust backend logic (commands + XML operations)     |
| `src-tauri/icons/` | Application icons for all platforms                |
| `doc/`             | Project documentation                              |
| `static/`          | Static assets served by SvelteKit                  |

---

## Component Tree

```mermaid
graph TD
    AppHtml["app.html\n(Shell)"]
    AppHtml --> LayoutSvelte["+layout.svelte\n(CSS import)"]
    LayoutSvelte --> PageSvelte["+page.svelte\n(Router)"]

    PageSvelte -->|"no file open"| LandingView["Landing View"]
    PageSvelte -->|"file open"| ViewerComp["Viewer"]

    subgraph LandingView["Landing View"]
        DropZone["DropZone\n• Drag & drop zone\n• File generation controls\n• Size presets"]
        RecentFiles["Recent Files Panel\n• File list from localStorage\n• Click to open\n• Remove entries"]
    end

    subgraph ViewerComp["Viewer"]
        HeaderComp["Header\n• Home button\n• File name\n• Jump first/last\n• Search bar\n• Cancel button\n• Prev/Next\n• XPath display"]
        ContentArea["Content Area"]
    end

    ContentArea -->|"has active element"| ThreePanel["Three-Panel View"]
    ContentArea -->|"no active element"| SingleChunk["Single Chunk View"]

    subgraph ThreePanel["Three-Panel View"]
        Before["Context Before\n(~2KB, dimmed)"]
        Active["Active Element\n(amber highlight,\npretty-printed)"]
        After["Context After\n(~2KB, dimmed)"]
    end

    Before --> Active --> After

    style PageSvelte fill:#1e1e2e,stroke:#89b4fa,color:#cdd6f4
    style HeaderComp fill:#1e1e2e,stroke:#f9e2af,color:#cdd6f4
    style Active fill:#451a03,stroke:#fbbf24,color:#fbbf24
```

### Routing Logic

The app uses a single page (`+page.svelte`) that conditionally renders based on `appState.currentFile`:

```mermaid
stateDiagram-v2
    [*] --> Landing : App starts
    Landing --> Viewer : openFile(path)
    Viewer --> Landing : closeFile()
    Landing --> Landing : removeFromRecentFiles()
    Landing --> Generating : generateSampleFile()
    Generating --> Viewer : Generation complete
```

---

## IPC Command Map

All communication between frontend and backend uses Tauri's `invoke()` IPC mechanism. Commands are defined in `xml_ops.rs` and registered in `lib.rs`.

```mermaid
sequenceDiagram
    participant UI as Frontend (Svelte)
    participant IPC as Tauri IPC
    participant BE as Backend (Rust)
    participant FS as File System

    Note over UI,FS: File Operations
    UI->>IPC: invoke("open_file", {path})
    IPC->>BE: open_file(path)
    BE->>FS: File::open + metadata
    FS-->>BE: file length (u64)
    BE-->>IPC: Ok(file_size)
    IPC-->>UI: file_size

    UI->>IPC: invoke("read_chunk", {path, offset, size})
    IPC->>BE: read_chunk(path, offset, size)
    BE->>FS: Seek + Read
    FS-->>BE: raw bytes
    BE-->>IPC: Ok(utf8_string)
    IPC-->>UI: chunk text

    Note over UI,FS: Search Operations
    UI->>IPC: invoke("search_node", {path, query, startOffset})
    IPC->>BE: search_node(path, query, start_offset)
    BE->>FS: Stream with quick-xml Reader
    FS-->>BE: XML events
    BE-->>IPC: Ok(SearchResult)
    IPC-->>UI: {found, xpath, element_text, ...}

    Note over UI,FS: Cancellation
    UI->>IPC: invoke("cancel_search")
    IPC->>BE: cancel_search()
    BE->>BE: SEARCH_CANCELLED.store(true)
    BE-->>IPC: Ok(())
```

### Command Reference

```mermaid
graph LR
    subgraph Commands["Tauri Commands (7 total)"]
        direction TB
        GenXml["generate_large_xml\n(path, size_mb, depth)"]
        OpenFile["open_file\n(path) → u64"]
        ReadChunk["read_chunk\n(path, offset, size) → String"]
        SearchNode["search_node\n(path, query, start_offset)\n→ SearchResult"]
        CancelSearch["cancel_search\n() → ()"]
        GetFirst["get_first_child\n(path) → SearchResult"]
        GetLast["get_last_child\n(path) → SearchResult"]
    end

    subgraph Categories
        direction TB
        FileIO["📂 File I/O"]
        Search["🔍 Search"]
        Nav["🧭 Navigation"]
        Gen["🏭 Generation"]
    end

    FileIO --- OpenFile
    FileIO --- ReadChunk
    Search --- SearchNode
    Search --- CancelSearch
    Nav --- GetFirst
    Nav --- GetLast
    Gen --- GenXml

    style Commands fill:#1e1e2e,stroke:#6c7086,color:#cdd6f4
```

---

## State Management

The entire application state lives in a single `AppState` class instance (`src/lib/state.svelte.ts`) using Svelte 5 runes for fine-grained reactivity.

```mermaid
classDiagram
    class AppState {
        +string|null currentFile
        +number fileSize
        +number viewOffset
        +boolean isSearching
        +boolean isLoadingElement
        +string contentBefore
        +string contentActive
        +string contentAfter
        +string contentWindow
        +string searchQuery
        +number|null lastMatchOffset
        +string currentXpath
        +boolean searchNotFound
        +RecentFile[] recentFiles
        +string fileName$
        +openFile(path) void
        +closeFile() void
        +loadChunk() void
        +performSearch(query, next) void
        +cancelSearch() void
        +goToStart() void
        +goToEnd() void
        +generateSampleFile(sizeMb, depth) void
        +removeFromRecentFiles(path) void
        -addToRecentFiles(path) void
        -updateViewFromResult(result) void
        -loadThreeSections(start, end) void
    }

    class RecentFile {
        +string path
        +string name
        +number openedAt
    }

    class SearchResult {
        +boolean found
        +string xpath
        +string element_text
        +string context_before
        +string context_after
        +number offset
    }

    AppState --> "0..*" RecentFile : recentFiles
    AppState ..> SearchResult : receives from backend
```

### State Flow

```mermaid
graph TD
    subgraph Reactive["Svelte 5 Reactive State ($state)"]
        CurrentFile["currentFile"]
        Content["contentBefore\ncontentActive\ncontentAfter"]
        SearchState["isSearching\nsearchNotFound\nsearchQuery"]
        XPath["currentXpath"]
        ViewOffset["viewOffset\nlastMatchOffset"]
    end

    subgraph Derived["Derived State ($derived)"]
        FileName["fileName"]
        HasActive["hasActiveContent"]
        Highlighted["highlightedBefore\nhighlightedActive\nhighlightedAfter"]
    end

    subgraph Persistence["localStorage"]
        RecentFiles["recentFiles[]"]
    end

    CurrentFile --> FileName
    Content --> HasActive
    Content --> Highlighted

    style Reactive fill:#1e1e2e,stroke:#89b4fa,color:#cdd6f4
    style Derived fill:#1e1e2e,stroke:#a6e3a1,color:#cdd6f4
    style Persistence fill:#1e1e2e,stroke:#f9e2af,color:#cdd6f4
```

---

## Search Lifecycle

The search system supports forward/backward navigation and cooperative cancellation.

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Searching : performSearch(query, next)
    Searching --> Found : result.found == true
    Searching --> NotFound : result.found == false
    Searching --> Cancelled : cancelSearch()

    Found --> Idle : Update view + XPath
    NotFound --> FlashWarning : Show "Not found" in XPath box
    FlashWarning --> Idle : Auto-dismiss (2s timeout)
    Cancelled --> Idle : Reset isSearching

    state Searching {
        [*] --> ResetCancelFlag
        ResetCancelFlag --> StreamXML
        StreamXML --> CheckCancel : Each event
        CheckCancel --> StreamXML : Not cancelled
        CheckCancel --> EarlyReturn : SEARCH_CANCELLED == true
        StreamXML --> MatchFound : element_matches() == true
        StreamXML --> EOF : End of file
    }
```

### Search Matching Pipeline

```mermaid
graph TD
    Event["XML Event\n(Start or Empty)"]
    Event --> CheckOffset{"offset >= start_offset?"}
    CheckOffset -->|No| Skip["Skip, continue"]
    CheckOffset -->|Yes| MatchTag{"Tag name\ncontains query?"}
    MatchTag -->|Yes| Match["✅ MATCH"]
    MatchTag -->|No| MatchId{"id attr\ncontains query?"}
    MatchId -->|Yes| Match
    MatchId -->|No| MatchGuid{"guid attr\ncontains query?"}
    MatchGuid -->|Yes| Match
    MatchGuid -->|No| MatchName{"name attr\ncontains query?"}
    MatchName -->|Yes| Match
    MatchName -->|No| Skip

    Match --> FindEnd["find_element_end_pos()"]
    FindEnd --> Extract["extract_and_build_result()"]
    Extract --> Result["SearchResult\n{found, xpath, element_text,\ncontext_before, context_after, offset}"]

    style Match fill:#166534,stroke:#22c55e,color:#fff
    style Skip fill:#1e1e2e,stroke:#6c7086,color:#cdd6f4
```

### Cancellation Mechanism

```mermaid
sequenceDiagram
    participant UI as Frontend
    participant SearchThread as Search Thread
    participant Flag as AtomicBool (SEARCH_CANCELLED)
    participant CancelThread as Cancel Handler

    UI->>SearchThread: invoke("search_node")
    SearchThread->>Flag: store(false)

    loop Every XML event
        SearchThread->>Flag: load()
        Flag-->>SearchThread: false
        SearchThread->>SearchThread: Process event
    end

    UI->>CancelThread: invoke("cancel_search")
    CancelThread->>Flag: store(true)
    CancelThread-->>UI: Ok(())

    SearchThread->>Flag: load()
    Flag-->>SearchThread: true
    SearchThread-->>UI: SearchResult { found: false }
```

---

## File I/O Strategy

The application **never** loads an entire file into memory. All file access uses offset-based seeking.

```mermaid
graph TB
    subgraph File["XML File on Disk (up to 2GB+)"]
        Region1["Bytes 0 .. N"]
        Region2["..."]
        Region3["Bytes M .. EOF"]
    end

    subgraph Strategies["I/O Strategies"]
        direction TB

        subgraph ChunkRead["Chunk Read (read_chunk)"]
            CR["Seek to offset\nRead N bytes\nReturn UTF-8 string"]
        end

        subgraph ForwardScan["Forward Scan (search, first_child)"]
            FS["Stream from offset 0\nParse XML events\nStop at match"]
        end

        subgraph ReverseScan["Reverse Scan (last_child)"]
            RS["Read 64KB chunks\nfrom end of file\nParse tags backward\nTrack depth counter"]
        end

        subgraph ContextExtract["Context Extraction"]
            CE["Scan back 128B for '<'\nScan fwd 128B for '>'\nRead element text\nRead ~2KB before\nRead ~2KB after"]
        end
    end

    Region1 -.- ForwardScan
    Region2 -.- ChunkRead
    Region3 -.- ReverseScan
    ForwardScan --> ContextExtract
    ReverseScan --> ContextExtract

    style File fill:#1e1e2e,stroke:#fab387,color:#cdd6f4
```

### Memory Profile

```mermaid
pie title Memory Usage per Operation
    "XML Event Buffer" : 10
    "Stack (XPath tracking)" : 5
    "Context Before (~2KB)" : 20
    "Context After (~2KB)" : 20
    "Element Text (variable)" : 30
    "Scan Buffer (64KB for reverse)" : 15
```

> The total memory footprint stays well under **1MB** regardless of file size. The largest allocation is the 64KB chunk buffer used by the reverse scanner for `get_last_child`.

---

## Navigation Flow

```mermaid
graph LR
    subgraph User Actions
        JumpFirst["⏮ Jump to First"]
        JumpLast["⏭ Jump to Last"]
        SearchNext["▶ Search Next"]
        SearchPrev["◀ Search Prev"]
        Cancel["✕ Cancel"]
    end

    subgraph Backend Commands
        GetFirst["get_first_child"]
        GetLast["get_last_child"]
        SearchFwd["search_node\n(offset = last + 1)"]
        SearchBwd["search_node\n(offset = 0)"]
        CancelCmd["cancel_search"]
    end

    subgraph View Update
        UpdateView["updateViewFromResult()\n• contentBefore\n• contentActive\n• contentAfter\n• viewOffset\n• currentXpath"]
    end

    JumpFirst --> GetFirst --> UpdateView
    JumpLast --> GetLast --> UpdateView
    SearchNext --> SearchFwd --> UpdateView
    SearchPrev --> SearchBwd --> UpdateView
    Cancel --> CancelCmd

    style UpdateView fill:#1e1e2e,stroke:#a6e3a1,color:#cdd6f4
```

---

## Backend Module Structure

```mermaid
graph TD
    subgraph lib.rs["lib.rs (Entry Point)"]
        TauriBuilder["tauri::Builder\n• Plugin: opener\n• Commands: 7 registered"]
    end

    subgraph xml_ops.rs["xml_ops.rs (Core Module)"]
        subgraph GlobalState["Global State"]
            AtomicFlag["static SEARCH_CANCELLED:\nAtomicBool"]
        end

        subgraph PublicAPI["Public Commands (#[tauri::command])"]
            GenXml["generate_large_xml"]
            OpenFile["open_file"]
            ReadChunk["read_chunk"]
            SearchNode["search_node"]
            CancelSearch["cancel_search"]
            FirstChild["get_first_child"]
            LastChild["get_last_child"]
        end

        subgraph Internals["Internal Functions"]
            GenInternal["generate_xml_internal"]
            WriteNested["write_nested_element"]
            PseudoUUID["pseudo_uuid"]
            ItemName["item_name"]
            ReadChunkInt["read_chunk_internal"]
            SearchInt["search_node_internal"]
            FirstInt["get_first_child_internal"]
            LastInt["get_last_child_internal"]
            ElemMatch["element_matches"]
            FindEnd["find_element_end_pos"]
            ExtractResult["extract_and_build_result"]
            ClassifyTag["classify_tag"]
            ExtractName["extract_tag_name_from_bytes"]
        end

        subgraph DataTypes["Data Types"]
            SearchResult2["SearchResult (struct)"]
            TagKind["TagKind (enum)\n• Open\n• Close\n• Empty"]
        end
    end

    TauriBuilder --> PublicAPI

    GenXml --> GenInternal
    GenInternal --> WriteNested
    WriteNested --> PseudoUUID
    WriteNested --> ItemName

    ReadChunk --> ReadChunkInt
    SearchNode --> SearchInt
    SearchInt --> ElemMatch
    SearchInt --> FindEnd
    SearchInt --> ExtractResult
    SearchNode -.-> AtomicFlag
    CancelSearch -.-> AtomicFlag

    FirstChild --> FirstInt
    FirstInt --> FindEnd
    FirstInt --> ExtractResult

    LastChild --> LastInt
    LastInt --> ClassifyTag
    ClassifyTag --> ExtractName
    LastInt --> ExtractResult

    style GlobalState fill:#451a03,stroke:#fbbf24,color:#fbbf24
    style PublicAPI fill:#1e1e2e,stroke:#89b4fa,color:#cdd6f4
    style Internals fill:#1e1e2e,stroke:#6c7086,color:#cdd6f4
```

---

## Technology Stack

```mermaid
graph BT
    subgraph Platform["Runtime"]
        Tauri2["Tauri 2.x"]
        WebView["WebView2 (Windows)"]
    end

    subgraph FrontendStack["Frontend"]
        Svelte5["Svelte 5"]
        SvelteKit["SvelteKit\n(SPA mode, SSR disabled)"]
        TW4["Tailwind CSS 4"]
        Vite6["Vite 6"]
    end

    subgraph BackendStack["Backend"]
        Rust["Rust 2021"]
        QuickXml2["quick-xml 0.31"]
        Anyhow["anyhow 1.0"]
        Serde["serde + serde_json"]
    end

    Svelte5 --> SvelteKit --> Vite6 --> Tauri2
    TW4 --> Vite6
    Rust --> Tauri2
    QuickXml2 --> Rust
    Anyhow --> Rust
    Serde --> Rust
    Tauri2 --> WebView

    style Platform fill:#1e1e2e,stroke:#fab387,color:#cdd6f4
    style FrontendStack fill:#1e1e2e,stroke:#89b4fa,color:#cdd6f4
    style BackendStack fill:#1e1e2e,stroke:#a6e3a1,color:#cdd6f4
```
