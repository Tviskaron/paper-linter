import { gunzipSync, unzipSync } from "fflate";
import untar from "js-untar";
import init, { PaperLinter } from "../pkg/paper_linter.js";
import "./styles.css";

const MAX_UPLOAD_BYTES = 250 * 1024 * 1024;
const EXCLUDED_PATH_PARTS = new Set([
  ".cache",
  ".git",
  ".github",
  ".idea",
  ".next",
  ".nuxt",
  ".venv",
  "__pycache__",
  "__macosx",
  "build",
  "dist",
  "node_modules",
  "out",
  "target",
  "venv",
]);
const EXCLUDED_PATH_PREFIXES = ["web/pkg/", "web/dist/"];
const EXCLUDED_FILE_EXTENSIONS = new Set([
  ".aux",
  ".bbl",
  ".bcf",
  ".blg",
  ".fls",
  ".fdb_latexmk",
  ".log",
  ".out",
  ".pdf",
  ".synctex.gz",
  ".xdv",
]);

type RuleView = {
  code: string;
  severity: "error" | "warning";
  name: string;
  summary: string;
  enabled_by_default: boolean;
  strict_only: boolean;
  family: string;
};

type Diagnostic = {
  code: string;
  severity: "error" | "warning";
  message: string;
  hint?: string;
  file: string;
  line: number;
  column: number;
};

type CheckOutput = {
  error?: string;
  diagnostics?: Diagnostic[];
  checked_files?: string[];
  active_view_id?: string;
  views?: ReportView[];
  summary?: {
    files_checked: number;
    errors: number;
    warnings: number;
  };
};

type ReportView = {
  id: string;
  kind: "root" | "all";
  label: string;
  root?: string;
  file_count: number;
  reason: string;
  preferred: boolean;
};

type LoadedFile = {
  path: string;
  bytes: Uint8Array;
};

const form = byId<HTMLFormElement>("lint-form");
const statusEl = byId("status");
const ruleGroupsEl = byId("rule-groups");
const filterEl = byId<HTMLInputElement>("rule-filter");
const selectValueEl = byId<HTMLInputElement>("select-value");
const rulesCountEl = byId("rules-count");
const reportEl = byId("report");
const copyReportBtn = byId<HTMLButtonElement>("copy-report");
const profileOptionsEl = byId("profile-options");
const profileClearBtn = byId<HTMLButtonElement>("profile-clear");
const profileResetBtn = byId<HTMLButtonElement>("profile-reset");
const themeToggle = byId<HTMLButtonElement>("theme-toggle");
const reportTabsEl = byId("report-tabs");
const archiveInput = byId<HTMLInputElement>("archive-input");
const directoryInput = byId<HTMLInputElement>("directory-input");
const dropZone = byId("drop-zone");
const dropName = byId("drop-name");
const chooseArchiveBtn = byId<HTMLButtonElement>("choose-archive");
const chooseDirectoryBtn = byId<HTMLButtonElement>("choose-directory");

let rules: RuleView[] = [];
let selectedRuleCodes = new Set<string>();
let loadedFiles: LoadedFile[] = [];
let sourceLabel = "";
let lastReportMarkdown = "";
let selectedReportViewId: string | null = null;
let availableReportViews: ReportView[] = [];
let rerunTimer: number | undefined;
let runSequence = 0;
let runInProgress = false;
const expandedFamilies = new Set<string>();

type ProfileName = "essential" | "standard" | "strict" | "polish";

const presetProfiles = {
  essential: {
    enable: ["CIT012", "PKG001"],
    disable: ["CIT002"],
    strict: false,
  },
  standard: {
    enable: ["PKG001", "PKG002", "SEC006", "CAP002", "TEX002", "SYN001"],
    disable: ["CIT002", "PRJ004"],
    strict: false,
  },
  strict: {
    enable: ["PKG001", "PKG002", "SEC006", "CAP002", "CIT011", "SYN001"],
    disable: ["CIT002"],
    strict: true,
  },
  polish: {
    enable: ["TXT001", "TXT003", "TXT004", "SEC001"],
    disable: ["CIT002", "PRJ004"],
    strict: false,
  },
} satisfies Record<ProfileName, { enable: string[]; disable: string[]; strict: boolean }>;

let activeProfile: ProfileName = "standard";
let profileModified = false;

const profileDescriptions: Record<ProfileName, string> = {
  essential: "Fast build-breaking and portability checks.",
  standard: "Balanced paper hygiene for regular authoring.",
  strict: "Broader submission checks, including stricter citation cleanup.",
  polish: "Light prose and presentation cleanup.",
};

const groupDescriptions: Record<string, string> = {
  ALG: "Algorithm hygiene: algorithm labels should be connected to the paper text and not left orphaned.",
  AUX: "Compiler auxiliary state: unresolved citations or references found in generated aux files.",
  BIB: "Bibliography quality: identifiers, required metadata, duplicates, and private local-only fields.",
  BLG: "BibTeX processing: bibliography errors reported by generated .blg logs.",
  CAP: "Caption quality: figure and table captions should exist and use expected punctuation.",
  CIT: "Citation integrity: citation keys, bibliography reachability, citation style, and citation punctuation.",
  ENV: "LaTeX environments: begin/end pairs should be balanced and structurally consistent.",
  FIG: "Figure integrity: assets, labels, references, paths, formats, and basic image validity.",
  FMT: "Source formatting: final newlines, repeated blank lines, and similar low-level file formatting.",
  LAT: "LaTeX style: legacy commands and low-level TeX primitives in ordinary LaTeX sources.",
  LBL: "Label hygiene: labels should be reachable and referenced where appropriate.",
  LOG: "LaTeX compile logs: errors and unresolved-reference warnings emitted by the compiler.",
  MTH: "Math notation style: display delimiters, scripts, and common operators inside math mode.",
  PKG: "Package usage: option clashes, risky package order, and missing package dependencies.",
  PRJ: "Project structure: root discovery, includes, reachable TeX files, and orphan source files.",
  RDY: "Submission readiness: compile and PDF regressions against a prepared baseline.",
  REF: "Reference integrity: references should point to labels that actually exist.",
  SEC: "Section structure: hierarchy, empty sections, singleton subdivisions, and heading style.",
  SYN: "Source syntax: preamble-level syntax problems such as unbalanced braces.",
  TAB: "Table hygiene: table labels and references should be present and connected to text.",
  TEX: "TeX typography and references: non-breaking spaces before refs/cites and hard-coded reference numbers.",
  TXT: "Prose cleanup: placeholders, repeated words, long sentences, filler words, and passive phrasing.",
  WS: "Whitespace cleanup: trailing spaces and tabs at line endings.",
};

const groupLabels: Record<string, string> = {
  ALG: "Algorithms",
  AUX: "Compiler Aux",
  BIB: "Bibliography",
  BLG: "BibTeX Logs",
  CAP: "Captions",
  CIT: "Citations",
  ENV: "Environments",
  FIG: "Figures",
  FMT: "Formatting",
  LAT: "LaTeX Style",
  LBL: "Labels",
  LOG: "Compile Logs",
  MTH: "Math",
  PKG: "Packages",
  PRJ: "Project",
  RDY: "Readiness",
  REF: "References",
  SEC: "Sections",
  SYN: "Syntax",
  TAB: "Tables",
  TEX: "TeX Typography",
  TXT: "Prose",
  WS: "Whitespace",
};

void boot();

async function boot() {
  await init();
  const linter = new PaperLinter();
  const data = JSON.parse(linter.rules_json()) as { rules: RuleView[] };
  rules = data.rules;
  setTheme(localStorage.getItem("paper-linter-theme") || "light");
  bindEvents();
  renderRuleGroups();
  applyProfileSelection(activeProfile);
  statusEl.textContent = "waiting for source";
}

function bindEvents() {
  themeToggle.addEventListener("click", () => {
    setTheme(document.body.dataset.theme === "dark" ? "light" : "dark");
  });
  archiveInput.addEventListener("change", async () => {
    const file = archiveInput.files?.[0];
    if (file) await withSourceLoadError(() => setArchiveFile(file));
  });
  directoryInput.addEventListener("change", async () => {
    const files = Array.from(directoryInput.files ?? []);
    if (files.length > 0) await withSourceLoadError(() => setDirectoryFiles(files));
  });

  bindDropZone(dropZone);
  chooseArchiveBtn.addEventListener("click", () => archiveInput.click());
  chooseDirectoryBtn.addEventListener("click", openDirectory);

  document.addEventListener("dragover", (event) => event.preventDefault());
  document.addEventListener("drop", async (event) => {
    if ((event.target as Element | null)?.closest("#drop-zone")) return;
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) await withSourceLoadError(() => setArchiveFile(file));
  });

  profileOptionsEl.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>("[data-profile]");
    const profile = button?.dataset.profile as ProfileName | undefined;
    if (!profile || !presetProfiles[profile]) return;
    applyProfileSelection(profile);
    scheduleAutoRun();
  });
  profileResetBtn.addEventListener("click", () => {
    applyProfileSelection(activeProfile);
    scheduleAutoRun();
  });
  profileClearBtn.addEventListener("click", () => {
    selectedRuleCodes.clear();
    markProfileModified();
    syncSelectionState();
    scheduleAutoRun();
  });
  filterEl.addEventListener("input", renderRuleGroups);
  ruleGroupsEl.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>("[data-family-toggle]");
    if (!button) return;
    if ((event.target as Element | null)?.matches('input[type="checkbox"]')) return;
    const family = button.dataset.familyToggle;
    if (!family) return;
    if (expandedFamilies.has(family)) expandedFamilies.delete(family);
    else expandedFamilies.add(family);
    renderRuleGroups();
  });
  ruleGroupsEl.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    if (!input.matches('input[type="checkbox"]')) return;

    if (input.dataset.family) {
      toggleFamilySelection(input.dataset.family);
    } else if (input.dataset.ruleCode) {
      markProfileModified();
      if (input.checked) selectedRuleCodes.add(input.dataset.ruleCode);
      else selectedRuleCodes.delete(input.dataset.ruleCode);
    }

    syncSelectionState();
    scheduleAutoRun();
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    scheduleAutoRun(0);
  });
  copyReportBtn.addEventListener("click", () => {
    void copyReport();
  });
  reportTabsEl.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>("[data-view-id]");
    if (!button || button.dataset.viewId === selectedReportViewId) return;
    selectedReportViewId = button.dataset.viewId ?? null;
    renderReportTabs();
    runNow();
  });
}

function setTheme(theme: string) {
  document.body.dataset.theme = theme;
  themeToggle.textContent = theme === "dark" ? "Light mode" : "Dark mode";
  themeToggle.setAttribute("aria-pressed", String(theme === "dark"));
  localStorage.setItem("paper-linter-theme", theme);
}

async function setArchiveFile(file: File) {
  statusEl.textContent = "reading archive...";
  loadedFiles = prepareLoadedFiles(await extractArchive(file));
  resetReportViews();
  sourceLabel = `uploaded archive ${file.name}`;
  dropName.textContent = `${file.name} (${formatBytes(file.size)})`;
  dropZone.classList.add("has-file");
  statusEl.textContent = `${loadedFiles.length} file(s) ready`;
  runNow();
}

async function setDirectoryFiles(files: File[]) {
  statusEl.textContent = "reading directory...";
  const loaded = await Promise.all(
    files.map(async (file) => ({
      path: file.webkitRelativePath || file.name,
      bytes: new Uint8Array(await file.arrayBuffer()),
    })),
  );
  loadedFiles = prepareLoadedFiles(loaded);
  resetReportViews();
  sourceLabel = `uploaded directory ${commonPrefixLabel(files)}`;
  dropName.textContent = `${commonPrefixLabel(files)} (${loadedFiles.length} files)`;
  dropZone.classList.add("has-file");
  statusEl.textContent = `${loadedFiles.length} file(s) ready`;
  runNow();
}

async function openDirectory() {
  if (!window.showDirectoryPicker) {
    directoryInput.click();
    return;
  }
  try {
    statusEl.textContent = "reading directory...";
    const handle = await window.showDirectoryPicker({ mode: "read" });
    const loaded = await readDirectoryHandle(handle);
    loadedFiles = prepareLoadedFiles(loaded);
    resetReportViews();
    sourceLabel = `opened directory ${handle.name}`;
    dropName.textContent = `${handle.name} (${loadedFiles.length} files)`;
    dropZone.classList.add("has-file");
    statusEl.textContent = `${loadedFiles.length} file(s) ready`;
    runNow();
  } catch (error) {
    if (isAbortError(error)) {
      statusEl.textContent = loadedFiles.length > 0 ? `${loadedFiles.length} file(s) ready` : "ready";
      return;
    }
    reportEl.textContent = error instanceof Error ? error.message : String(error);
    reportEl.classList.remove("empty");
    statusEl.textContent = "failed";
  }
}

async function readDirectoryHandle(handle: FileSystemDirectoryHandle, prefix = handle.name): Promise<LoadedFile[]> {
  const files: LoadedFile[] = [];
  for await (const [name, entry] of handle.entries()) {
    const path = `${prefix}/${name}`;
    if (entry.kind === "directory") {
      files.push(...(await readDirectoryHandle(entry, path)));
      continue;
    }
    const file = await entry.getFile();
    files.push({ path, bytes: new Uint8Array(await file.arrayBuffer()) });
  }
  return files;
}

function bindDropZone(zone: HTMLElement) {
  ["dragenter", "dragover"].forEach((eventName) => {
    zone.addEventListener(eventName, (event) => {
      event.preventDefault();
      event.stopPropagation();
      zone.classList.add("dragging");
    });
  });
  ["dragleave", "drop"].forEach((eventName) => {
    zone.addEventListener(eventName, (event) => {
      event.preventDefault();
      event.stopPropagation();
      zone.classList.remove("dragging");
    });
  });
  zone.addEventListener("drop", async (event) => {
    const dragEvent = event as DragEvent;
    const directoryFiles = await droppedDirectoryFiles(dragEvent.dataTransfer);
    if (directoryFiles.length > 0) {
      await withSourceLoadError(() => setDroppedDirectoryFiles(directoryFiles));
      return;
    }
    const file = dragEvent.dataTransfer?.files?.[0];
    if (file) await withSourceLoadError(() => setArchiveFile(file));
  });
}

function activateFileInput(event: KeyboardEvent, input: HTMLInputElement) {
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  input.click();
}

async function activateDirectoryInput(event: KeyboardEvent) {
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  await openDirectory();
}

async function setDroppedDirectoryFiles(files: LoadedFile[]) {
  statusEl.textContent = "reading directory...";
  const root = files[0]?.path.split("/")[0] || "directory";
  loadedFiles = prepareLoadedFiles(files);
  resetReportViews();
  sourceLabel = `dropped directory ${root}`;
  dropName.textContent = `${root} (${loadedFiles.length} files)`;
  dropZone.classList.add("has-file");
  statusEl.textContent = `${loadedFiles.length} file(s) ready`;
  runNow();
}

async function droppedDirectoryFiles(dataTransfer: DataTransfer | null): Promise<LoadedFile[]> {
  const items = Array.from(dataTransfer?.items ?? []);
  const entries = items
    .map((item) => (item as DataTransferItem & { webkitGetAsEntry?: () => unknown }).webkitGetAsEntry?.())
    .filter(Boolean);
  if (!entries.some((entry) => isDroppedDirectoryEntry(entry))) return [];
  const files: LoadedFile[] = [];
  for (const entry of entries) {
    files.push(...(await readDroppedEntry(entry, "")));
  }
  return files;
}

async function readDroppedEntry(entry: unknown, prefix: string): Promise<LoadedFile[]> {
  const item = entry as {
    isFile?: boolean;
    isDirectory?: boolean;
    name?: string;
    file?: (success: (file: File) => void, error: (error: unknown) => void) => void;
    createReader?: () => { readEntries: (success: (entries: unknown[]) => void, error: (error: unknown) => void) => void };
  };
  const name = item.name || "";
  const path = [prefix, name].filter(Boolean).join("/");
  if (item.isFile && item.file) {
    const file = await new Promise<File>((resolve, reject) => item.file?.(resolve, reject));
    return [{ path, bytes: new Uint8Array(await file.arrayBuffer()) }];
  }
  if (!item.isDirectory || !item.createReader) return [];
  const entries = await readAllDroppedEntries(item.createReader());
  const files: LoadedFile[] = [];
  for (const child of entries) {
    files.push(...(await readDroppedEntry(child, path)));
  }
  return files;
}

async function readAllDroppedEntries(reader: { readEntries: (success: (entries: unknown[]) => void, error: (error: unknown) => void) => void }) {
  const entries: unknown[] = [];
  while (true) {
    const batch = await new Promise<unknown[]>((resolve, reject) => reader.readEntries(resolve, reject));
    if (batch.length === 0) return entries;
    entries.push(...batch);
  }
}

function isDroppedDirectoryEntry(entry: unknown) {
  return Boolean((entry as { isDirectory?: boolean }).isDirectory);
}

async function withSourceLoadError(action: () => Promise<void>) {
  try {
    await action();
  } catch (error) {
    reportEl.textContent = error instanceof Error ? error.message : String(error);
    reportEl.classList.remove("empty");
    lastReportMarkdown = "";
    copyReportBtn.disabled = true;
    statusEl.textContent = "failed";
  }
}

async function extractArchive(file: File): Promise<LoadedFile[]> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const lower = file.name.toLowerCase();
  if (lower.endsWith(".zip") || bytes[0] === 0x50 && bytes[1] === 0x4b) {
    return Object.entries(unzipSync(bytes))
      .filter(([path]) => !path.endsWith("/"))
      .map(([path, data]) => ({ path, bytes: data }));
  }
  if (lower.endsWith(".tar.gz") || lower.endsWith(".tgz") || lower.endsWith(".gz") || bytes[0] === 0x1f && bytes[1] === 0x8b) {
    const decoded = gunzipSync(bytes);
    if (looksLikeTar(decoded)) return unpackTar(decoded);
    return [{ path: "source.tex", bytes: decoded }];
  }
  if (lower.endsWith(".tar")) return unpackTar(bytes);
  throw new Error(`unsupported archive format for '${file.name}'; use .zip, .tar, .tar.gz, or .tgz`);
}

async function unpackTar(bytes: Uint8Array): Promise<LoadedFile[]> {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  const records = await untar(buffer);
  const files: LoadedFile[] = [];
  for (const record of records) {
    if (record.name.endsWith("/")) continue;
    const data = record.buffer ?? (await record.blob?.arrayBuffer());
    if (data) files.push({ path: record.name, bytes: new Uint8Array(data) });
  }
  return files;
}

function looksLikeTar(bytes: Uint8Array) {
  if (bytes.length <= 512) return false;
  const marker = new TextDecoder().decode(bytes.slice(257, 262));
  return marker === "ustar" || bytes.slice(0, 512).some((byte) => byte === 0);
}

function scheduleAutoRun(delay = 250) {
  if (loadedFiles.length === 0) {
    statusEl.textContent = "waiting for source";
    return;
  }
  if (!runInProgress) {
    runNow();
    return;
  }
  runSequence += 1;
  if (rerunTimer !== undefined) window.clearTimeout(rerunTimer);
  rerunTimer = window.setTimeout(() => {
    rerunTimer = undefined;
    runNow();
  }, delay);
}

function runNow() {
  if (loadedFiles.length === 0) {
    statusEl.textContent = "waiting for source";
    return;
  }
  if (runInProgress) {
    scheduleAutoRun();
    return;
  }
  const runId = ++runSequence;
  void runLinter(runId);
}

async function runLinter(runId: number) {
  if (loadedFiles.length === 0) {
    statusEl.textContent = "waiting for source";
    return;
  }
  runInProgress = true;
  syncSelectedRules();
  let runningShown = false;
  const runningTimer = window.setTimeout(() => {
    if (runId !== runSequence) return;
    runningShown = true;
    statusEl.textContent = "running...";
    reportEl.textContent = "Running linter...";
    reportEl.classList.add("empty");
  }, 250);
  lastReportMarkdown = "";
  copyReportBtn.disabled = true;

  try {
    const total = loadedFiles.reduce((sum, file) => sum + file.bytes.length, 0);
    enforceLimit(total);
    const linter = new PaperLinter();
    for (const file of loadedFiles) {
      linter.add_file(file.path, file.bytes);
    }
    const output = JSON.parse(
      linter.check(
        JSON.stringify({
          preset: profileModified ? null : activeProfile,
          select: [...selectedRuleCodes].sort(),
          ignore: [],
          strict: profileModified ? false : presetProfiles[activeProfile].strict,
          all_rules: false,
          ...selectedViewOptions(),
        }),
      ),
    ) as CheckOutput;
    if (output.error) throw new Error(output.error);
    if (runId !== runSequence) return;
    renderResult(output);
    statusEl.textContent = "";
  } catch (error) {
    if (runId !== runSequence) return;
    reportEl.textContent = error instanceof Error ? error.message : String(error);
    reportEl.classList.remove("empty");
    lastReportMarkdown = "";
    copyReportBtn.disabled = true;
    statusEl.textContent = "failed";
  } finally {
    window.clearTimeout(runningTimer);
    if (!runningShown && runId === runSequence) statusEl.textContent = statusEl.textContent === "failed" ? "failed" : "";
    runInProgress = false;
  }
}

function renderResult(output: CheckOutput) {
  const diagnostics = output.diagnostics ?? [];
  const checkedFiles = output.checked_files ?? [];
  availableReportViews = output.views ?? [];
  selectedReportViewId = output.active_view_id ?? selectedReportViewId;
  const filesChecked = output.summary?.files_checked ?? checkedFiles.length;
  const errors = output.summary?.errors ?? diagnostics.filter((diagnostic) => diagnostic.severity === "error").length;
  const warnings = output.summary?.warnings ?? diagnostics.filter((diagnostic) => diagnostic.severity === "warning").length;
  byId("source").textContent = sourceLabel;
  byId("files").textContent = `files ${filesChecked}`;
  byId("errors").textContent = `errors ${errors}`;
  byId("warnings").textContent = `warnings ${warnings}`;
  renderReportTabs();
  lastReportMarkdown = renderReportMarkdown(diagnostics, checkedFiles, filesChecked, errors, warnings, activeReportView());
  reportEl.innerHTML = renderMarkdown(lastReportMarkdown);
  reportEl.classList.remove("empty");
  copyReportBtn.disabled = false;
}

function selectedViewOptions() {
  const selected = availableReportViews.find((view) => view.id === selectedReportViewId);
  if (!selected) return { all_tex: false, root: null };
  if (selected.kind === "all") return { all_tex: true, root: null };
  return { all_tex: false, root: selected.root ?? null };
}

function activeReportView() {
  return availableReportViews.find((view) => view.id === selectedReportViewId);
}

function resetReportViews() {
  selectedReportViewId = null;
  availableReportViews = [];
  renderReportTabs();
}

function renderReportTabs() {
  if (availableReportViews.length <= 1) {
    reportTabsEl.innerHTML = "";
    reportTabsEl.hidden = true;
    return;
  }

  reportTabsEl.hidden = false;
  reportTabsEl.innerHTML = availableReportViews.map((view) => {
    const selected = view.id === selectedReportViewId;
    const count = `${view.file_count} file${view.file_count === 1 ? "" : "s"}`;
    const title = `${view.reason}; ${count}`;
    return `
      <button class="report-tab ${selected ? "active" : ""}" type="button" data-view-id="${escapeHtml(view.id)}" aria-pressed="${selected}" title="${escapeHtml(title)}">
        <span>${escapeHtml(view.label)}</span>
        <small>${count}</small>
      </button>
    `;
  }).join("");
}

async function copyReport() {
  if (!lastReportMarkdown) return;
  try {
    await navigator.clipboard.writeText(lastReportMarkdown);
    const originalTitle = copyReportBtn.title;
    copyReportBtn.classList.add("copied");
    copyReportBtn.title = "Copied!";
    window.setTimeout(() => {
      copyReportBtn.classList.remove("copied");
      copyReportBtn.title = originalTitle;
    }, 1600);
  } catch {
    statusEl.textContent = "copy failed";
  }
}

function renderRuleGroups() {
  const filter = filterEl.value.trim().toLowerCase();
  ruleGroupsEl.innerHTML = "";
  let visibleGroups = 0;

  for (const [family, familyRules] of groupedRules()) {
    const title = groupLabel(family);
    const description = groupDescription(family);
    const familyMatches = filter.length > 0 && `${family} ${title} ${description}`.toLowerCase().includes(filter);
    const matchingRules = filter ? familyRules.filter((rule) => ruleMatchesFilter(rule, filter)) : familyRules;
    if (filter && !familyMatches && matchingRules.length === 0) continue;

    const selectedCount = familyRules.filter((rule) => selectedRuleCodes.has(rule.code)).length;
    const state = selectedCount === familyRules.length ? "all" : selectedCount > 0 ? "partial" : "none";
    const canExpand = familyRules.length > 1;
    const expanded = canExpand && (filter.length > 0 || expandedFamilies.has(family));
    const visibleRules = filter && !familyMatches ? matchingRules : familyRules;
    const group = document.createElement("section");
    group.className = `inspection-group ${state} ${expanded ? "expanded" : ""}`;
    group.dataset.family = family;
    const familyControl = canExpand ? `
      <div class="inspection-family">
        <label class="family-check-zone" aria-label="Toggle ${escapeHtml(title)} rules">
          <input class="family-check" type="checkbox" data-family="${escapeHtml(family)}">
        </label>
        <button class="family-toggle" type="button" data-family-toggle="${escapeHtml(family)}" aria-expanded="${expanded}" aria-controls="rules-${escapeHtml(family)}">
          <span class="family-copy">
            <span class="family-title"><span class="family-chevron" aria-hidden="true"></span><code>${escapeHtml(family)}</code>${escapeHtml(title)}</span>
            <span class="family-description">${escapeHtml(description)}</span>
          </span>
          <span class="family-count">${selectedCount}/${familyRules.length}</span>
        </button>
      </div>
    ` : `
      <label class="inspection-family single-family">
        <span class="family-check-zone" aria-hidden="true">
          <input class="family-check" type="checkbox" data-family="${escapeHtml(family)}">
        </span>
        <span class="family-toggle single">
          <span class="family-copy">
            <span class="family-title"><code>${escapeHtml(family)}</code>${escapeHtml(title)}</span>
            <span class="family-description">${escapeHtml(description)}</span>
          </span>
          <span class="family-count">${selectedCount}/${familyRules.length}</span>
        </span>
      </label>
    `;
    group.innerHTML = `
      ${familyControl}
      <div id="rules-${escapeHtml(family)}" class="inspection-rules" ${expanded ? "" : "hidden"}>
        ${visibleRules.map(renderRuleRow).join("")}
      </div>
    `;
    ruleGroupsEl.appendChild(group);
    visibleGroups += 1;
  }

  if (visibleGroups === 0) {
    const empty = document.createElement("div");
    empty.className = "inspection-empty";
    empty.textContent = "No rules match this search.";
    ruleGroupsEl.appendChild(empty);
  }

  syncSelectionState();
}

function renderRuleRow(rule: RuleView) {
  const selected = selectedRuleCodes.has(rule.code);
  return `
    <label class="inspection-rule ${rule.severity} ${selected ? "selected" : ""}" data-rule-family="${escapeHtml(rule.family)}">
      <input type="checkbox" data-rule-code="${escapeHtml(rule.code)}" ${selected ? "checked" : ""}>
      <code>${escapeHtml(rule.code)}</code>
      <span class="rule-copy">
        <span class="rule-name">${escapeHtml(rule.name)}</span>
        <span class="rule-summary">${escapeHtml(rule.summary)}</span>
      </span>
      <span class="rule-severity">${escapeHtml(rule.severity)}</span>
    </label>
  `;
}

function syncSelectedRules() {
  selectValueEl.value = [...selectedRuleCodes].sort().join(",");
}

function syncGroupCheckboxes() {
  for (const input of ruleGroupsEl.querySelectorAll<HTMLInputElement>(".family-check")) {
    const family = input.dataset.family ?? "";
    const familyRules = rules.filter((rule) => rule.family === family);
    const selectedCount = familyRules.filter((rule) => selectedRuleCodes.has(rule.code)).length;
    const allSelected = selectedCount > 0 && selectedCount === familyRules.length;
    const partiallySelected = selectedCount > 0 && selectedCount < familyRules.length;
    const row = input.closest(".inspection-group");
    input.checked = allSelected;
    input.indeterminate = partiallySelected;
    input.dataset.state = allSelected ? "all" : partiallySelected ? "partial" : "none";
    input.setAttribute("aria-checked", partiallySelected ? "mixed" : String(allSelected));
    row?.classList.toggle("all", allSelected);
    row?.classList.toggle("partial", partiallySelected);
    row?.classList.toggle("none", !allSelected && !partiallySelected);
    const count = row?.querySelector(".family-count");
    if (count) count.textContent = `${selectedCount}/${familyRules.length}`;
  }
}

function syncVisibleRuleCheckboxes() {
  for (const input of ruleGroupsEl.querySelectorAll<HTMLInputElement>("[data-rule-code]")) {
    const selected = selectedRuleCodes.has(input.dataset.ruleCode ?? "");
    input.checked = selected;
    input.closest(".inspection-rule")?.classList.toggle("selected", selected);
  }
}

function syncSelectionState() {
  syncSelectedRules();
  syncGroupCheckboxes();
  syncVisibleRuleCheckboxes();
  renderProfileState();
}

function toggleFamilySelection(family: string) {
  markProfileModified();
  const familyRules = rules.filter((rule) => rule.family === family);
  const selectedCount = familyRules.filter((rule) => selectedRuleCodes.has(rule.code)).length;
  const shouldEnable = selectedCount === 0;
  for (const rule of familyRules) {
    if (shouldEnable) selectedRuleCodes.add(rule.code);
    else selectedRuleCodes.delete(rule.code);
  }
}

function applyProfileSelection(name: ProfileName) {
  const profile = presetProfiles[name];
  if (!profile) return;
  activeProfile = name;
  profileModified = false;
  selectedRuleCodes = new Set(rules.filter((rule) => ruleEnabledByProfile(rule, profile)).map((rule) => rule.code));
  renderRuleGroups();
  syncSelectionState();
}

function renderProfileState() {
  for (const button of profileOptionsEl.querySelectorAll<HTMLButtonElement>("[data-profile]")) {
    const selected = button.dataset.profile === activeProfile;
    button.classList.toggle("active", selected);
    button.classList.toggle("modified", selected && profileModified);
    button.setAttribute("aria-pressed", String(selected));
    button.title = selected && profileModified ? `${profileDescriptions[activeProfile]} Modified` : "";
  }
  const selectedCount = selectedRuleCodes.size;
  rulesCountEl.textContent = `${selectedCount} selected`;
  rulesCountEl.title = profileDescriptions[activeProfile];
  profileClearBtn.textContent = "Clear all";
  profileClearBtn.disabled = selectedCount === 0;
  profileResetBtn.hidden = profileMatchesActiveProfile();
}

function ruleEnabledByProfile(rule: RuleView, profile: { enable: string[]; disable: string[]; strict: boolean }) {
  if (patternMatches(rule.code, profile.disable)) return false;
  if (patternMatches(rule.code, profile.enable)) return true;
  if (profile.strict && rule.strict_only) return true;
  return rule.enabled_by_default;
}

function patternMatches(code: string, patterns: string[]) {
  return patterns.some((pattern) => code.startsWith(pattern));
}

function markProfileModified() {
  profileModified = true;
  renderProfileState();
}

function profileMatchesActiveProfile() {
  const profile = presetProfiles[activeProfile];
  const profileCodes = rules.filter((rule) => ruleEnabledByProfile(rule, profile)).map((rule) => rule.code);
  if (profileCodes.length !== selectedRuleCodes.size) return false;
  return profileCodes.every((code) => selectedRuleCodes.has(code));
}

function profileLabel(profile: ProfileName) {
  return profile[0].toUpperCase() + profile.slice(1);
}

function groupedRules() {
  const groups = new Map<string, RuleView[]>();
  for (const rule of rules) {
    if (!groups.has(rule.family)) groups.set(rule.family, []);
    groups.get(rule.family)?.push(rule);
  }
  return [...groups.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function ruleMatchesFilter(rule: RuleView, filter: string) {
  return `${rule.code} ${rule.name} ${rule.summary} ${rule.severity}`.toLowerCase().includes(filter);
}

function groupLabel(family: string) {
  return groupLabels[family] || family;
}

function groupDescription(family: string) {
  return groupDescriptions[family] || "Checks related to this rule family.";
}

function renderReportMarkdown(diagnostics: Diagnostic[], checkedFiles: string[], filesChecked: number, errors: number, warnings: number, view?: ReportView) {
  let output = "# Paper Linter Report\n\n";
  output += "## Summary\n\n";
  output += `checked ${filesChecked} file(s), ${errors} error(s), ${warnings} warning(s)\n\n`;
  if (view) {
    output += `view ${view.kind === "all" ? "all .tex" : `\`${view.root ?? view.label}\``}\n\n`;
  }
  output += fileSummaryMarkdown(diagnostics, checkedFiles);
  if (diagnostics.length === 0) return output;

  output += "\n## By Severity\n\n| Severity | Count |\n|---|---:|\n";
  if (errors > 0) output += `| error | ${errors} |\n`;
  if (warnings > 0) output += `| warning | ${warnings} |\n`;

  const byCode = groupBy(diagnostics, (diagnostic) => diagnostic.code);
  const groups = [...byCode.entries()].sort(([leftCode, left], [rightCode, right]) => right.length - left.length || leftCode.localeCompare(rightCode));

  output += "\n## By Rule\n\n| Rule | Severity | Name | Count |\n|---|---|---|---:|\n";
  for (const [code, items] of groups) {
    output += `| \`${code}\` | ${items[0].severity} | ${markdownTableCell(ruleName(code))} | ${items.length} |\n`;
  }

  output += "\n## Diagnostics\n";
  for (const [code, items] of groups) {
    const severity = items[0].severity;
    const byMessage = groupBy(items, diagnosticMessageKey);
    if (byMessage.size === 1) {
      const [key, diagnosticsForMessage] = [...byMessage.entries()][0];
      output += `\n### ${formatMessageKey(key, severity, code)} (${diagnosticsForMessage.length})\n\n`;
      output += fileLocationsMarkdown(diagnosticsForMessage, 2, 4);
      continue;
    }
    output += `\n### ${severity}[${code}] ${ruleName(code)} (${items.length})\n`;
    for (const [key, diagnosticsForMessage] of byMessage) {
      output += `\n#### ${formatMessageKey(key, severity, code)}\n\n`;
      output += fileLocationsMarkdown(diagnosticsForMessage, 0, 2);
    }
  }
  return output;
}

function fileSummaryMarkdown(diagnostics: Diagnostic[], checkedFiles: string[]) {
  const counts = new Map<string, { errors: number; warnings: number }>();
  for (const file of checkedFiles) counts.set(file, { errors: 0, warnings: 0 });
  for (const diagnostic of diagnostics) {
    const count = counts.get(diagnostic.file) ?? { errors: 0, warnings: 0 };
    if (diagnostic.severity === "error") count.errors += 1;
    else count.warnings += 1;
    counts.set(diagnostic.file, count);
  }
  let output = "| File | Errors | Warnings |\n|---|---:|---:|\n";
  for (const [file, count] of [...counts.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    output += `| \`${markdownTableCell(file)}\` | ${count.errors} | ${count.warnings} |\n`;
  }
  return output;
}

function fileLocationsMarkdown(diagnostics: Diagnostic[], fileIndent: number, locationIndent: number) {
  let output = "";
  const byFile = groupBy(diagnostics, (diagnostic) => diagnostic.file);
  for (const [file, items] of [...byFile.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    output += `${" ".repeat(fileIndent)}- \`${file}\`\n`;
    for (const diagnostic of items) {
      output += `${" ".repeat(locationIndent)}- \`${diagnostic.line}:${diagnostic.column}\`\n`;
    }
  }
  return output;
}

function diagnosticMessageKey(diagnostic: Diagnostic) {
  return JSON.stringify([diagnostic.message, diagnostic.hint ?? null]);
}

function formatMessageKey(key: string, severity: string, code: string) {
  const [message, hint] = JSON.parse(key) as [string, string | null];
  return `${severity}[${code}] ${message}${hint ? `; hint: ${hint}` : ""}`;
}

function ruleName(code: string) {
  return rules.find((rule) => rule.code === code)?.name ?? "unknown rule";
}

function renderMarkdown(markdown: string) {
  const lines = markdown.split("\n");
  const html: string[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) {
      index += 1;
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      const level = heading[1].length;
      html.push(`<h${level}>${renderInline(heading[2])}</h${level}>`);
      index += 1;
      continue;
    }

    if (isTableStart(lines, index)) {
      const table = parseTable(lines, index);
      html.push(table.html);
      index = table.nextIndex;
      continue;
    }

    if (line.trimStart().startsWith("- ")) {
      const list = parseList(lines, index, leadingSpaces(line));
      html.push(list.html);
      index = list.nextIndex;
      continue;
    }

    const paragraph: string[] = [];
    while (index < lines.length && lines[index].trim()) {
      paragraph.push(lines[index].trim());
      index += 1;
    }
    html.push(`<p>${renderInline(paragraph.join(" "))}</p>`);
  }

  return html.join("\n");
}

function parseTable(lines: string[], start: number) {
  const header = splitTableRow(lines[start]);
  let index = start + 2;
  const rows: string[][] = [];
  while (index < lines.length && lines[index].trim().startsWith("|")) {
    rows.push(splitTableRow(lines[index]));
    index += 1;
  }
  const head = `<thead><tr>${header.map((cell) => `<th>${renderInline(cell)}</th>`).join("")}</tr></thead>`;
  const body = `<tbody>${rows.map((row) => `<tr>${row.map((cell) => `<td>${renderInline(cell)}</td>`).join("")}</tr>`).join("")}</tbody>`;
  return { html: `<table>${head}${body}</table>`, nextIndex: index };
}

function parseList(lines: string[], start: number, indent: number) {
  const items: string[] = [];
  let index = start;
  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) {
      index += 1;
      continue;
    }
    const currentIndent = leadingSpaces(line);
    const trimmed = line.trimStart();
    if (currentIndent < indent || !trimmed.startsWith("- ")) break;
    if (currentIndent > indent) {
      const nested = parseList(lines, index, currentIndent);
      if (items.length > 0) items[items.length - 1] += nested.html;
      else items.push(nested.html);
      index = nested.nextIndex;
      continue;
    }
    items.push(renderInline(trimmed.slice(2)));
    index += 1;
  }
  return { html: `<ul>${items.map((item) => `<li>${item}</li>`).join("")}</ul>`, nextIndex: index };
}

function isTableStart(lines: string[], index: number) {
  return lines[index]?.trim().startsWith("|") && lines[index + 1]?.trim().startsWith("|") && /^[\s|:-]+$/.test(lines[index + 1]);
}

function splitTableRow(line: string) {
  return line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((cell) => cell.trim());
}

function leadingSpaces(line: string) {
  return line.length - line.trimStart().length;
}

function renderInline(value: string) {
  const escaped = escapeHtml(value);
  return escaped.replace(/`([^`]+)`/g, "<code>$1</code>");
}

function prepareLoadedFiles(files: LoadedFile[]) {
  const filtered = files.filter((file) => shouldKeepSourceFile(file.path));
  if (filtered.length === 0) throw new Error("no usable source files found after excluding generated files");
  return stripCommonRoot(filtered);
}

function shouldKeepSourceFile(path: string) {
  const normalized = normalizePath(path);
  const lower = normalized.toLowerCase();
  if (EXCLUDED_PATH_PREFIXES.some((prefix) => lower.startsWith(prefix))) return false;
  const parts = lower.split("/");
  if (parts.some((part) => EXCLUDED_PATH_PARTS.has(part))) return false;
  if (parts.some((part) => part.startsWith("._"))) return false;
  return ![...EXCLUDED_FILE_EXTENSIONS].some((extension) => lower.endsWith(extension));
}

function stripCommonRoot(files: LoadedFile[]) {
  const total = files.reduce((sum, file) => sum + file.bytes.length, 0);
  enforceLimit(total);
  const parts = files.map((file) => file.path.replaceAll("\\", "/").split("/").filter(Boolean));
  const first = parts[0]?.[0];
  if (!first || parts.some((path) => path[0] !== first || path.length === 1)) {
    return files.map((file) => ({ ...file, path: normalizePath(file.path) }));
  }
  return files.map((file) => ({ ...file, path: normalizePath(file.path).split("/").slice(1).join("/") }));
}

function normalizePath(path: string) {
  return path.replaceAll("\\", "/").split("/").filter(Boolean).join("/");
}

function commonPrefixLabel(files: File[]) {
  const first = files[0]?.webkitRelativePath?.split("/")?.[0];
  return first || "directory";
}

function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}

function enforceLimit(bytes: number) {
  if (bytes > MAX_UPLOAD_BYTES) {
    throw new Error(`input exceeds the ${formatBytes(MAX_UPLOAD_BYTES)} limit after excluding generated files`);
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function groupBy<T>(items: T[], key: (item: T) => string) {
  const groups = new Map<string, T[]>();
  for (const item of items) {
    const groupKey = key(item);
    groups.set(groupKey, [...(groups.get(groupKey) ?? []), item]);
  }
  return groups;
}

function markdownTableCell(value: string) {
  return value.replaceAll("|", "\\|");
}

function byId<T extends HTMLElement = HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
