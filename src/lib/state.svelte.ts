import { invoke } from "@tauri-apps/api/core";
import { appLocalDataDir, join } from "@tauri-apps/api/path";
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
  searchType = $state<string>("guid");
  searchStartPercentage = $state<number>(0);

  // Three-section content
  contentBefore = $state<string>("");
  contentActive = $state<string>("");
  contentAfter = $state<string>("");

  // Legacy single-chunk (used before any search)
  contentWindow = $state<string>("");

  // Search
  searchQuery = $state<string>("");
  lastMatchOffset = $state<number | null>(null);
  currentXpath = $state<string>("");
  searchNotFound = $state<boolean>(false);
  private searchNotFoundTimer: ReturnType<typeof setTimeout> | null = null;

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
    this.searchQuery = "";
    this.lastMatchOffset = null;
    this.currentXpath = "";
    this.isSearching = false;
    this.isLoadingElement = false;
  }

  async goToStart() {
    if (!this.currentFile) return;
    this.isLoadingElement = true;
    try {
      const result: any = await invoke("get_first_child", {
        path: this.currentFile,
      });
      this.updateViewFromResult(result);
      this.currentXpath = result.xpath;
    } catch (e) {
      console.error("Failed to go to start:", e);
    } finally {
      this.isLoadingElement = false;
    }
  }

  async goToEnd() {
    if (!this.currentFile) return;
    this.isLoadingElement = true;
    try {
      const result: any = await invoke("get_last_child", {
        path: this.currentFile,
      });
      this.updateViewFromResult(result);
      this.currentXpath = result.xpath;
    } catch (e) {
      console.error("Failed to go to end:", e);
    } finally {
      this.isLoadingElement = false;
    }
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

  async openFile(path: string) {
    try {
      this.fileSize = await invoke("open_file", { path });
      this.currentFile = path;
      this.viewOffset = 0;
      this.searchQuery = "";
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

  async performSearch(query: string, next: boolean = true) {
    if (!this.currentFile || !query) return;

    this.isSearching = true;
    this.searchProgress = 0;
    this.searchQuery = query;

    try {
      let start = 0;
      if (this.lastMatchOffset !== null && next) {
        start = this.lastMatchOffset + 1;
        // When continuing, the progress reflects current position
        this.searchProgress = Math.floor((start / this.fileSize) * 100);
      } else {
        // Start from the specified percentage
        start = Math.floor(this.fileSize * this.searchStartPercentage);
        this.searchProgress = Math.floor(this.searchStartPercentage * 100);
      }

      const result: any = await invoke("search_node", {
        path: this.currentFile,
        query: query,
        searchType: this.searchType,
        startOffset: start,
      });

      if (result.found) {
        this.updateViewFromResult(result);
        this.currentXpath = result.xpath;
        // Update the start percentage input to reflect where we found the match
        if (this.fileSize > 0) {
          this.searchStartPercentage = Number((result.offset / this.fileSize).toFixed(4));
        }
      } else {
        if (this.searchNotFoundTimer) clearTimeout(this.searchNotFoundTimer);
        this.searchNotFound = true;
        this.searchNotFoundTimer = setTimeout(() => {
          this.searchNotFound = false;
        }, 2000);
      }
    } catch (e) {
      console.error("Search failed:", e);
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

  async generateSampleFile(sizeMb: number = 50, depth: number = 3) {
    console.log(`Starting sample generation (${sizeMb}MB, depth ${depth})...`);
    try {
      const baseDir = await appLocalDataDir();
      const fullPath = await join(baseDir, `sample_${sizeMb}_${depth}.xml`);

      console.log("Invoking generate_large_xml command at:", fullPath);
      await invoke("generate_large_xml", {
        path: fullPath,
        sizeMb,
        depth,
      });
      console.log("Generation complete. Opening file...");
      await this.openFile(fullPath);
      console.log("File opened successfully.");
    } catch (e) {
      console.error("Generation failed:", e);
      alert("Failed to generate file: " + e);
    }
  }
}

export const appState = new AppState();
