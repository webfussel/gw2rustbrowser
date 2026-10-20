import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { SvelteSet } from "svelte/reactivity";
import {
  api,
  type Category,
  type DatSummary,
  type ExportProgress,
  type FileInfo,
  type FindResult,
  type ScanProgress,
} from "./api";
import { errorMessage, naturalCompare } from "./format";

export interface Task {
  label: string;
  done: number;
  total: number;
  cancellable: boolean;
}

export interface Notice {
  kind: "info" | "error";
  text: string;
}

class BrowserState {
  summary = $state<DatSummary | null>(null);
  categories = $state.raw<Category[]>([]);
  selectedCategory = $state<number | null>(null);
  files = $state.raw<FileInfo[]>([]);
  searchQuery = $state("");
  searchResults = $state.raw<FindResult[] | null>(null);
  selectedMft = $state<number | null>(null);
  expanded = new SvelteSet<number>();
  task = $state<Task | null>(null);
  notice = $state<Notice | null>(null);

  /** Child category ids per parent (`-1` = root), naturally sorted. */
  children = $derived.by(() => {
    const map = new Map<number, number[]>();
    for (const c of this.categories) {
      const key = c.parent ?? -1;
      let list = map.get(key);
      if (!list) map.set(key, (list = []));
      list.push(c.id);
    }
    for (const list of map.values()) {
      list.sort((a, b) => naturalCompare(this.categories[a].name, this.categories[b].name));
    }
    return map;
  });

  /** Files shown in the list: search results, else the selected category. */
  visibleFiles = $derived<(FileInfo | FindResult)[]>(this.searchResults ?? this.files);

  categoryPath(id: number): string[] {
    const path: string[] = [];
    for (let c: Category | undefined = this.categories[id]; c; c = c.parent === null ? undefined : this.categories[c.parent]) {
      path.unshift(c.name);
    }
    return path;
  }
}

export const browser = new BrowserState();

export function notify(text: string, kind: Notice["kind"] = "info") {
  browser.notice = { kind, text };
}

export async function openArchive(path?: string) {
  if (!path) {
    const picked = await open({
      title: "Open Gw2.dat",
      multiple: false,
      directory: false,
      filters: [{ name: "Guild Wars 2 archive", extensions: ["dat"] }],
    });
    if (!picked) return;
    path = picked;
  }

  browser.task = { label: "Opening archive", done: 0, total: 0, cancellable: false };
  const unlisten = await listen<ScanProgress>("scan-progress", ({ payload }) => {
    if (browser.task && payload.total > 0) {
      browser.task = { ...browser.task, label: "Scanning files", done: payload.done, total: payload.total };
    }
  });
  try {
    const summary = await api.openDat(path);
    browser.summary = summary;
    browser.categories = await api.categories();
    browser.expanded.clear();
    browser.searchResults = null;
    browser.searchQuery = "";
    browser.selectedMft = null;
    browser.files = [];
    browser.selectedCategory = null;
    const scanned = summary.fromCache
      ? summary.rescanned > 0
        ? `index updated, ${summary.rescanned.toLocaleString()} changed files rescanned`
        : "index loaded from cache"
      : "archive scanned";
    notify(`${summary.files.toLocaleString()} files, ${scanned} in ${(summary.millis / 1000).toFixed(1)} s`);
  } catch (error) {
    notify(`Could not open ${path}: ${errorMessage(error)}`, "error");
  } finally {
    unlisten();
    browser.task = null;
  }
}

export async function selectCategory(id: number) {
  browser.searchResults = null;
  browser.selectedCategory = id;
  try {
    browser.files = await api.categoryFiles(id);
  } catch (error) {
    notify(errorMessage(error), "error");
  }
}

/** Expands all ancestors of a category and selects it. */
export async function revealCategory(id: number) {
  for (let c = browser.categories[id]; c && c.parent !== null; c = browser.categories[c.parent]) {
    browser.expanded.add(c.parent);
  }
  browser.selectedCategory = id;
  browser.files = await api.categoryFiles(id);
}

export async function search(query: string) {
  browser.searchQuery = query;
  if (!query.trim()) {
    browser.searchResults = null;
    return;
  }
  try {
    const results = await api.find(query);
    browser.searchResults = results;
    if (results.length === 0) notify(`Nothing found for “${query}”`);
    else if (results.length === 1) browser.selectedMft = results[0].mft;
  } catch (error) {
    notify(errorMessage(error), "error");
  }
}

export function clearSearch() {
  browser.searchQuery = "";
  browser.searchResults = null;
}

export async function exportOne(mft: number, converted: boolean) {
  try {
    const defaultPath = await api.suggestFileName(mft, converted);
    const extension = defaultPath.split(".").pop() ?? "bin";
    const path = await save({ defaultPath, filters: [{ name: extension.toUpperCase(), extensions: [extension] }] });
    if (!path) return;
    await api.exportFile(mft, path, converted);
    notify(`Exported to ${path}`);
  } catch (error) {
    notify(`Export failed: ${errorMessage(error)}`, "error");
  }
}

export async function exportMany(target: { category?: number; files?: number[] }, converted: boolean) {
  const directory = await open({ title: "Export to folder", directory: true, multiple: false });
  if (!directory) return;
  browser.task = { label: "Exporting", done: 0, total: 0, cancellable: true };
  const unlisten = await listen<ExportProgress>("export-progress", ({ payload }) => {
    if (browser.task) browser.task = { ...browser.task, done: payload.done, total: payload.total };
  });
  try {
    const result = await api.exportFiles({ ...target, directory, converted });
    const parts = [`${result.written.toLocaleString()} files exported to ${result.directory}`];
    if (result.failed) parts.push(`${result.failed} failed (${result.errors.slice(0, 3).join("; ")})`);
    if (result.cancelled) parts.push("cancelled");
    notify(parts.join(", "), result.failed ? "error" : "info");
  } catch (error) {
    notify(`Export failed: ${errorMessage(error)}`, "error");
  } finally {
    unlisten();
    browser.task = null;
  }
}
