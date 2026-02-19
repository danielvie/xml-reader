import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface RecentFile {
  path: string;
  name: string;
  openedAt: number;
}
// ... (rest of imports/interfaces)

// ...



const RECENT_FILES_KEY = "xml-reader-recent-files";
const MAX_RECENT_FILES = 10;
const SEARCH_MEMORY_KEY = "xml-reader-search-memory";
const DEFAULT_SEARCH_TYPE = "any";

interface SearchMemoryEntry {
  query: string;
  type: string;
}

export interface AncestorInfo {
  name: string;
  offset: number;
  line_number: number;
}

function loadSearchMemory(): Record<string, SearchMemoryEntry> {
  try {
    const raw = localStorage.getItem(SEARCH_MEMORY_KEY);
    if (!raw) return {};
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

function saveSearchMemory(memory: Record<string, SearchMemoryEntry>) {
  try {
    localStorage.setItem(SEARCH_MEMORY_KEY, JSON.stringify(memory));
  } catch {
    // storage full or unavailable — ignore
  }
}

function loadRecentFilesFromStorage(): RecentFile[] {
  try {
    const raw = localStorage.getItem(RECENT_FILES_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RecentFile[];
    return parsed
      .sort((a, b) => b.openedAt - a.openedAt)
      .slice(0, MAX_RECENT_FILES);
  } catch {
    return [];
  }
}

function saveRecentFilesToStorage(files: RecentFile[]) {
  try {
    localStorage.setItem(RECENT_FILES_KEY, JSON.stringify(files));
  } catch {
    // storage full or unavailable — ignore
  }
}

export class AppState {
  currentFile = $state<string | null>(null);
  fileSize = $state<number>(0);
  viewOffset = $state<number>(0);
  isSearching = $state<boolean>(false);
  isLoadingElement = $state<boolean>(false);
  searchProgress = $state<number>(0);
  searchType = $state<string>(DEFAULT_SEARCH_TYPE);

  // Three-section content
  contentBefore = $state<string>("");
  contentActive = $state<string>("");
  contentAfter = $state<string>("");

  // Legacy single-chunk (used before any search)
  contentWindow = $state<string>("");

  // Scroll request: Viewer reacts to changes
  scrollTarget = $state<"top" | "bottom">("top");
  scrollRequest = $state<number>(0);

  // Search
  searchQuery = $state<string>("");
  lastMatchOffset = $state<number | null>(null);
  currentXpath = $state<string>("");
  searchNotFound = $state<boolean>(false);
  private searchNotFoundTimer: ReturnType<typeof setTimeout> | null = null;

  // Ancestor segments from currentXpath (excludes the element itself)
  get xpathSegments(): { name: string; depth: number }[] {
    if (!this.currentXpath || !this.currentXpath.includes("/")) return [];
    const parts = this.currentXpath.split("/").filter(Boolean);
    if (parts.length <= 1) return [];
    // All segments except the last (which is the current element)
    return parts.slice(0, -1).map((name, i) => ({ name, depth: i }));
  }

  // Recent files
  recentFiles = $state<RecentFile[]>(loadRecentFilesFromStorage());

  get fileName() {
    if (!this.currentFile) return "";
    return this.currentFile.split(/[\\/]/).pop() || this.currentFile;
  }

  constructor() {
    this.setupListeners();
  }

  async setupListeners() {
    await listen<number>("search-progress", (event) => {
      this.searchProgress = event.payload;
    });
  }

  private addToRecentFiles(path: string) {
    const name = path.split(/[\\/]/).pop() || path;
    // Remove duplicate if exists
    let files = this.recentFiles.filter((f) => f.path !== path);
    // Add at the front
    files.unshift({ path, name, openedAt: Date.now() });
    // Trim to max
    files = files.slice(0, MAX_RECENT_FILES);
    this.recentFiles = files;
    saveRecentFilesToStorage(files);
  }

  removeFromRecentFiles(path: string) {
    this.recentFiles = this.recentFiles.filter((f) => f.path !== path);
    saveRecentFilesToStorage(this.recentFiles);
  }

  closeFile() {
    this.currentFile = null;
    this.fileSize = 0;
    this.viewOffset = 0;
    this.contentWindow = "";
    this.contentBefore = "";
    this.contentActive = "";
    this.contentAfter = "";
    this.lastMatchOffset = null;
    this.currentXpath = "";
    this.isSearching = false;
    this.isLoadingElement = false;
    // searchQuery and searchType are intentionally preserved
  }

  clearSearch() {
    this.searchQuery = "";
    this.searchType = DEFAULT_SEARCH_TYPE;
    this.lastMatchOffset = null;
    this.saveSearchPrefs();
  }

  saveSearchPrefs() {
    if (!this.currentFile) return;
    const memory = loadSearchMemory();
    memory[this.currentFile] = {
      query: this.searchQuery,
      type: this.searchType,
    };
    saveSearchMemory(memory);
  }

  private loadSearchPrefsForFile(path: string) {
    const memory = loadSearchMemory();
    const entry = memory[path];
    if (entry) {
      this.searchQuery = entry.query;
      this.searchType = entry.type;
    } else {
      this.searchQuery = "";
      this.searchType = DEFAULT_SEARCH_TYPE;
    }
  }

  focusTop() {
    this.scrollTarget = "top";
    this.scrollRequest++;
  }

  focusBottom() {
    this.scrollTarget = "bottom";
    this.scrollRequest++;
  }

  currentLineNumber = $state<number | null>(null);
  ancestors = $state<AncestorInfo[]>([]);

  // Helper to update state from a SearchResult
  private updateViewFromResult(result: any) {
    this.lastMatchOffset = result.offset;
    this.viewOffset = result.offset;
    this.contentBefore = result.context_before;
    this.contentActive = result.element_text;
    this.contentAfter = result.context_after;
    this.currentLineNumber = result.line_number || null;
    this.ancestors = result.ancestors || [];
    this.contentWindow =
      this.contentBefore + this.contentActive + this.contentAfter;
  }

  async navigateToAncestor(depth: number) {
    if (!this.currentFile) return;
    this.isLoadingElement = true;

    // Check if we have cached ancestor info
    if (this.ancestors[depth]) {
      const ancestor = this.ancestors[depth];
      // Use cached offset
      try {
        const result: any = await invoke("read_element_at_offset", {
            path: this.currentFile,
            offset: ancestor.offset,
        });
        
        // When we jump to an ancestor, the new ancestors list is a prefix of the old one.
        // The backend might return empty ancestors for read_element_at_offset (as implemented),
        // so we should reconstruct or preserve the known ancestors.
        // Actually, logic: if we go to depth K, the new ancestors are 0..K-1.
        // The current element (at depth K) becomes the active one.
        // So we can manually fix up the state if backend doesn't return ancestors.
        
        if (result.ancestors && result.ancestors.length === 0) {
            // Reconstruct ancestors from our cache
            result.ancestors = this.ancestors.slice(0, depth);
            // Result xpath might be ".../name", we might want to fix it if needed,
            // but the frontend reconstruction in xpathSegments getter relies on currentXpath string.
            // The backend read_element_at_offset returns xpath ending in /name.
            // We should ideally ensure currentXpath is correct.
            // Let's rely on what we know:
            // currentXpath should be /Ancestors.../Target
        }
        
        this.updateViewFromResult(result);
        
        // Fix up xpath if backend returned partial
        if (result.xpath.startsWith("...")) {
            // Reconstruct full xpath
             const parts = this.ancestors.slice(0, depth).map(a => a.name);
             // And add current element name (which might handle indices or not? Backend handles indices...)
             // Wait, if we use read_element_at_offset, backend returns generic xpath like ".../name".
             // We lose the index info like "Create[3]".
             // But we have the full xpath in `this.ancestors`? No, `AncestorInfo` only has `name`.
             // Actually, `this.currentXpath` has the full string!
             // We can just slice `this.currentXpath`.
             const segments = this.currentXpath.split("/").filter(Boolean);
             // We want segments 0..depth.
             if (segments.length > depth) {
                 const newPath = "/" + segments.slice(0, depth + 1).join("/");
                 this.currentXpath = newPath;
             }
        }

        this.focusTop();
      } catch (e) {
        console.error("Optimized nav failed, falling back", e);
        this.fallbackNavigateToAncestor(depth);
      } finally {
        this.isLoadingElement = false;
      }
      return;
    }

    this.fallbackNavigateToAncestor(depth);
  }

  private async fallbackNavigateToAncestor(depth: number) {
    if (!this.currentFile || this.lastMatchOffset === null) {
        this.isLoadingElement = false; 
        return; 
    }
    
    try {
      const result: any = await invoke("find_parent", {
        path: this.currentFile,
        childOffset: this.lastMatchOffset,
        ancestorDepth: depth,
      });
      if (result.found) {
        this.updateViewFromResult(result);
        this.currentXpath = result.xpath;
        this.focusTop();
      }
    } catch (e) {
      console.error(e);
    } finally {
      this.isLoadingElement = false;
    }
  }

  async openFile(path: string) {
    try {
      this.fileSize = await invoke("open_file", { path });
      this.currentFile = path;
      this.viewOffset = 0;
      this.lastMatchOffset = null;
      this.currentXpath = "";
      this.contentBefore = "";
      this.contentActive = "";
      this.contentAfter = "";
      this.addToRecentFiles(path);
      this.loadSearchPrefsForFile(path);
      await this.loadChunk();
    } catch (e) {
      console.error("Failed to open file:", e);
    }
  }

  async loadChunk() {
    if (!this.currentFile) return;
    try {
      const chunkSize = 5000;
      const text = await invoke<string>("read_chunk", {
        path: this.currentFile,
        offset: this.viewOffset,
        size: chunkSize,
      });
      this.contentWindow = text;
    } catch (e) {
      console.error("Failed to read chunk:", e);
    }
  }

  async performSearch(query: string, next: boolean = false) {
    if (!this.currentFile || !query) return;

    this.isSearching = true;
    this.searchProgress = 0;
    this.searchQuery = query;
    this.saveSearchPrefs();

    // Capture current XPath to restore if not found
    const previousXpath = this.currentXpath;
    this.currentXpath = "Searching...";

    try {
      let start = 0;
      if (this.lastMatchOffset !== null && next) {
        start = this.lastMatchOffset + 1;
        this.searchProgress = Math.floor((start / this.fileSize) * 100);
      }

      const result: any = await invoke("search_node", {
        path: this.currentFile,
        query: query,
        searchType: this.searchType,
        startOffset: start,
      });

      if (result.found) {
        this.updateViewFromResult(result);

        if (start > 0) {
          this.currentXpath = "Constructing XPath...";
          const tagName = result.xpath.replace(/^\//, "");

          invoke<string>("resolve_xpath", {
            path: this.currentFile,
            offset: result.offset,
            tagName
          })
            .then((fullPath) => {
              this.currentXpath = fullPath;
            })
            .catch((e) => {
              console.error("Failed to resolve XPath:", e);
              this.currentXpath = "XPath lookup failed";
            });
        } else {
          this.currentXpath = result.xpath;
        }
      } else {
        if (this.searchNotFoundTimer) clearTimeout(this.searchNotFoundTimer);
        this.searchNotFound = true;
        this.currentXpath = "Not Found";
        this.searchNotFoundTimer = setTimeout(() => {
          this.searchNotFound = false;
          this.currentXpath = previousXpath;
        }, 2000);
      }
    } catch (e) {
      console.error("Search failed:", e);
      this.currentXpath = "Error";
    } finally {
      this.isSearching = false;
      this.searchProgress = 0;
    }
  }

  async cancelSearch() {
    try {
      await invoke("cancel_search");
    } catch (e) {
      console.error("Cancel search failed:", e);
    } finally {
      this.isSearching = false;
    }
  }

  async loadThreeSections(startOffset: number, endOffset?: number) {
    if (!this.currentFile) return;

    const beforeSize = 2000;
    const afterSize = 2000;
    const activeSize = endOffset ? endOffset - startOffset : 1000;

    try {
      const beforeStart = Math.max(0, startOffset - beforeSize);
      const actualBeforeSize = startOffset - beforeStart;
      if (actualBeforeSize > 0) {
        this.contentBefore = await invoke<string>("read_chunk", {
          path: this.currentFile,
          offset: beforeStart,
          size: actualBeforeSize,
        });
      } else {
        this.contentBefore = "";
      }

      this.contentActive = await invoke<string>("read_chunk", {
        path: this.currentFile,
        offset: startOffset,
        size: activeSize,
      });

      const afterStart = startOffset + activeSize;
      this.contentAfter = await invoke<string>("read_chunk", {
        path: this.currentFile,
        offset: afterStart,
        size: afterSize,
      });

      this.contentWindow =
        this.contentBefore + this.contentActive + this.contentAfter;
    } catch (e) {
      console.error("Failed to load sections:", e);
    }
  }
}

export const appState = new AppState();
