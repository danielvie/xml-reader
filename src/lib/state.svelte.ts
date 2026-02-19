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
const SEARCH_TYPE_KEY = "xml-reader-search-type";
const SEARCH_QUERY_KEY = "xml-reader-search-query";
const DEFAULT_SEARCH_TYPE = "tag";

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
  searchType = $state<string>(localStorage.getItem(SEARCH_TYPE_KEY) || DEFAULT_SEARCH_TYPE);

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
  searchQuery = $state<string>(localStorage.getItem(SEARCH_QUERY_KEY) || "");
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
    try {
      localStorage.setItem(SEARCH_TYPE_KEY, this.searchType);
      localStorage.setItem(SEARCH_QUERY_KEY, this.searchQuery);
    } catch {
      // storage full or unavailable — ignore
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

  // Helper to update state from a SearchResult
  private updateViewFromResult(result: any) {
    this.lastMatchOffset = result.offset;
    this.viewOffset = result.offset;
    this.contentBefore = result.context_before;
    this.contentActive = result.element_text;
    this.contentAfter = result.context_after;
    this.contentWindow =
      this.contentBefore + this.contentActive + this.contentAfter;
  }

  async navigateToAncestor(depth: number) {
    if (!this.currentFile || this.lastMatchOffset === null) return;
    this.isLoadingElement = true;
    try {
      const result: any = await invoke("find_parent", {
        path: this.currentFile,
        childOffset: this.lastMatchOffset,
        ancestorDepth: depth,
      });
      if (result.found) {
        this.updateViewFromResult(result);
        this.currentXpath = result.xpath;
      }
    } catch (e) {
      console.error("Failed to navigate to ancestor:", e);
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
