import { gunzipSync, unzipSync } from "fflate";
import untar from "js-untar";
import init, { PaperLinter } from "../pkg/paper_linter.js";
import "./styles.css";

const MAX_UPLOAD_BYTES = 100 * 1024 * 1024;

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
  summary?: {
    files_checked: number;
    errors: number;
    warnings: number;
  };
};

type LoadedFile = {
  path: string;
  bytes: Uint8Array;
};

const form = byId<HTMLFormElement>("lint-form");
const statusEl = byId("status");
const rulesEl = byId("rules");
const ruleGroupsEl = byId("rule-groups");
const filterEl = byId<HTMLInputElement>("rule-filter");
const selectValueEl = byId<HTMLInputElement>("select-value");
const reportEl = byId("report");
const presetSelect = byId<HTMLSelectElement>("preset-select");
const themeToggle = byId<HTMLButtonElement>("theme-toggle");
const allTexToggle = byId<HTMLInputElement>("all-tex-toggle");
const archivePanel = byId("archive-panel");
const directoryPanel = byId("directory-panel");
const archiveInput = byId<HTMLInputElement>("archive-input");
const directoryInput = byId<HTMLInputElement>("directory-input");
const dropZone = byId("drop-zone");
const directoryZone = byId("directory-zone");
const dropName = byId("drop-name");
const directoryName = byId("directory-name");

let rules: RuleView[] = [];
let selectedRuleCodes = new Set<string>();
let loadedFiles: LoadedFile[] = [];
let sourceLabel = "";

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
} satisfies Record<string, { enable: string[]; disable: string[]; strict: boolean }>;

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

void boot();

async function boot() {
  await init();
  const linter = new PaperLinter();
  const data = JSON.parse(linter.rules_json()) as { rules: RuleView[] };
  rules = data.rules;
  setTheme(localStorage.getItem("paper-linter-theme") || "dark");
  bindEvents();
  renderRuleGroups();
  renderRules();
  applyPresetSelection(presetSelect.value);
  statusEl.textContent = "ready";
}

function bindEvents() {
  themeToggle.addEventListener("click", () => {
    setTheme(document.body.dataset.theme === "dark" ? "light" : "dark");
  });
  allTexToggle.addEventListener("change", syncTexMode);
  syncTexMode();

  document.querySelectorAll<HTMLElement>("[data-mode]").forEach((button) => {
    button.addEventListener("click", () => setInputMode(button.dataset.mode ?? "archive"));
  });

  archiveInput.addEventListener("change", async () => {
    const file = archiveInput.files?.[0];
    if (file) await setArchiveFile(file);
  });
  directoryInput.addEventListener("change", async () => {
    const files = Array.from(directoryInput.files ?? []);
    if (files.length > 0) await setDirectoryFiles(files);
  });

  bindDropZone(dropZone, (file) => setArchiveFile(file));
  dropZone.addEventListener("click", () => archiveInput.click());
  dropZone.addEventListener("keydown", (event) => activateFileInput(event, archiveInput));

  directoryZone.addEventListener("click", openDirectory);
  directoryZone.addEventListener("keydown", (event) => activateDirectoryInput(event));

  document.addEventListener("dragover", (event) => event.preventDefault());
  document.addEventListener("drop", async (event) => {
    if ((event.target as Element | null)?.closest("#drop-zone")) return;
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) await setArchiveFile(file);
  });

  presetSelect.addEventListener("change", () => {
    if (presetSelect.value !== "custom") applyPresetSelection(presetSelect.value);
  });
  filterEl.addEventListener("input", renderRules);
  rulesEl.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    if (!input.matches('input[type="checkbox"]')) return;
    markCustomPreset();
    if (input.checked) selectedRuleCodes.add(input.value);
    else selectedRuleCodes.delete(input.value);
    syncSelectionState();
  });
  ruleGroupsEl.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    if (!input.matches('input[type="checkbox"]')) return;
    markCustomPreset();
    const familyRules = rules.filter((rule) => rule.family === input.value);
    const selectedCount = familyRules.filter((rule) => selectedRuleCodes.has(rule.code)).length;
    const shouldEnable = selectedCount < familyRules.length;
    for (const rule of familyRules) {
      if (shouldEnable) selectedRuleCodes.add(rule.code);
      else selectedRuleCodes.delete(rule.code);
    }
    syncSelectionState();
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await runLinter();
  });
}

function setTheme(theme: string) {
  document.body.dataset.theme = theme;
  themeToggle.textContent = theme === "dark" ? "Light mode" : "Dark mode";
  themeToggle.setAttribute("aria-pressed", String(theme === "dark"));
  localStorage.setItem("paper-linter-theme", theme);
}

function syncTexMode() {
  document.querySelector('[data-mode-label="filtered"]')?.classList.toggle("active", !allTexToggle.checked);
  document.querySelector('[data-mode-label="all"]')?.classList.toggle("active", allTexToggle.checked);
}

function setInputMode(mode: string) {
  const directory = mode === "directory";
  document.querySelectorAll<HTMLElement>("[data-mode]").forEach((item) => {
    item.classList.toggle("active", item.dataset.mode === mode);
  });
  archivePanel.hidden = directory;
  directoryPanel.hidden = !directory;
}

async function setArchiveFile(file: File) {
  setInputMode("archive");
  statusEl.textContent = "reading archive...";
  loadedFiles = stripCommonRoot(await extractArchive(file));
  sourceLabel = `uploaded archive ${file.name}`;
  dropName.textContent = `${file.name} (${formatBytes(file.size)})`;
  dropZone.classList.add("has-file");
  statusEl.textContent = `${loadedFiles.length} file(s) ready`;
}

async function setDirectoryFiles(files: File[]) {
  setInputMode("directory");
  statusEl.textContent = "reading directory...";
  const loaded = await Promise.all(
    files.map(async (file) => ({
      path: file.webkitRelativePath || file.name,
      bytes: new Uint8Array(await file.arrayBuffer()),
    })),
  );
  loadedFiles = stripCommonRoot(loaded);
  sourceLabel = `uploaded directory ${commonPrefixLabel(files)}`;
  directoryName.textContent = `${commonPrefixLabel(files)} (${loadedFiles.length} files)`;
  directoryZone.classList.add("has-file");
  statusEl.textContent = `${loadedFiles.length} file(s) ready`;
}

async function openDirectory() {
  if (!window.showDirectoryPicker) {
    directoryInput.click();
    return;
  }
  try {
    setInputMode("directory");
    statusEl.textContent = "reading directory...";
    const handle = await window.showDirectoryPicker({ mode: "read" });
    const loaded = await readDirectoryHandle(handle);
    loadedFiles = stripCommonRoot(loaded);
    sourceLabel = `opened directory ${handle.name}`;
    directoryName.textContent = `${handle.name} (${loadedFiles.length} files)`;
    directoryZone.classList.add("has-file");
    statusEl.textContent = `${loadedFiles.length} file(s) ready`;
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

function bindDropZone(zone: HTMLElement, onFile: (file: File) => Promise<void>) {
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
    const file = dragEvent.dataTransfer?.files?.[0];
    if (file) await onFile(file);
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

async function extractArchive(file: File): Promise<LoadedFile[]> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  enforceLimit(bytes.length);
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

async function runLinter() {
  if (loadedFiles.length === 0) {
    statusEl.textContent = "choose an archive or directory";
    return;
  }
  syncSelectedRules();
  statusEl.textContent = "running...";
  reportEl.textContent = "Running linter...";
  reportEl.classList.add("empty");

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
          preset: presetSelect.value === "custom" ? null : presetSelect.value,
          select: [...selectedRuleCodes].sort(),
          ignore: [],
          strict: presetProfiles[presetSelect.value as keyof typeof presetProfiles]?.strict ?? false,
          all_rules: false,
          all_tex: allTexToggle.checked,
        }),
      ),
    ) as CheckOutput;
    if (output.error) throw new Error(output.error);
    renderResult(output);
    statusEl.textContent = "done";
  } catch (error) {
    reportEl.textContent = error instanceof Error ? error.message : String(error);
    reportEl.classList.remove("empty");
    statusEl.textContent = "failed";
  }
}

function renderResult(output: CheckOutput) {
  const diagnostics = output.diagnostics ?? [];
  const checkedFiles = output.checked_files ?? [];
  const filesChecked = output.summary?.files_checked ?? checkedFiles.length;
  const errors = output.summary?.errors ?? diagnostics.filter((diagnostic) => diagnostic.severity === "error").length;
  const warnings = output.summary?.warnings ?? diagnostics.filter((diagnostic) => diagnostic.severity === "warning").length;
  byId("source").textContent = sourceLabel;
  byId("files").textContent = `files ${filesChecked}`;
  byId("errors").textContent = `errors ${errors}`;
  byId("warnings").textContent = `warnings ${warnings}`;
  reportEl.innerHTML = renderMarkdown(renderReportMarkdown(diagnostics, checkedFiles, filesChecked, errors, warnings));
  reportEl.classList.remove("empty");
}

function renderRules() {
  const filter = filterEl.value.trim().toLowerCase();
  rulesEl.innerHTML = "";
  for (const rule of rules) {
    const haystack = `${rule.code} ${rule.name} ${rule.summary}`.toLowerCase();
    if (filter && !haystack.includes(filter)) continue;
    const label = document.createElement("label");
    label.className = "rule";
    label.innerHTML = `
      <input type="checkbox" value="${rule.code}" ${selectedRuleCodes.has(rule.code) ? "checked" : ""}>
      <code>${rule.code}</code>
      <span>${escapeHtml(rule.name)}<small>${escapeHtml(rule.summary)}</small></span>
    `;
    rulesEl.appendChild(label);
  }
  syncSelectionState();
}

function renderRuleGroups() {
  ruleGroupsEl.innerHTML = "";
  for (const [family, familyRules] of groupedRules()) {
    const label = document.createElement("label");
    const description = groupDescription(family);
    label.className = "group-row";
    label.dataset.tooltip = description;
    label.setAttribute("aria-label", `${family}: ${description}`);
    label.innerHTML = `
      <input type="checkbox" value="${family}">
      <span>${family}</span>
      <small>${familyRules.length}</small>
    `;
    ruleGroupsEl.appendChild(label);
  }
  syncSelectionState();
}

function syncSelectedRules() {
  selectValueEl.value = [...selectedRuleCodes].sort().join(",");
}

function syncGroupCheckboxes() {
  for (const input of ruleGroupsEl.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')) {
    const familyRules = rules.filter((rule) => rule.family === input.value);
    const selectedCount = familyRules.filter((rule) => selectedRuleCodes.has(rule.code)).length;
    const allSelected = selectedCount > 0 && selectedCount === familyRules.length;
    const partiallySelected = selectedCount > 0 && selectedCount < familyRules.length;
    const row = input.closest(".group-row");
    input.checked = allSelected;
    input.indeterminate = partiallySelected;
    input.dataset.state = allSelected ? "all" : partiallySelected ? "partial" : "none";
    input.setAttribute("aria-checked", partiallySelected ? "mixed" : String(allSelected));
    row?.classList.toggle("all", allSelected);
    row?.classList.toggle("partial", partiallySelected);
    row?.classList.toggle("none", !allSelected && !partiallySelected);
    const small = row?.querySelector("small");
    if (small) small.textContent = `${selectedCount}/${familyRules.length}`;
  }
}

function syncVisibleRuleCheckboxes() {
  for (const input of rulesEl.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')) {
    input.checked = selectedRuleCodes.has(input.value);
  }
}

function syncSelectionState() {
  syncSelectedRules();
  syncGroupCheckboxes();
  syncVisibleRuleCheckboxes();
}

function applyPresetSelection(name: string) {
  const profile = presetProfiles[name as keyof typeof presetProfiles];
  if (!profile) return;
  selectedRuleCodes = new Set(rules.filter((rule) => ruleEnabledByProfile(rule, profile)).map((rule) => rule.code));
  renderRules();
  renderRuleGroups();
  syncSelectionState();
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

function markCustomPreset() {
  if (presetSelect.value !== "custom") presetSelect.value = "custom";
}

function groupedRules() {
  const groups = new Map<string, RuleView[]>();
  for (const rule of rules) {
    if (!groups.has(rule.family)) groups.set(rule.family, []);
    groups.get(rule.family)?.push(rule);
  }
  return [...groups.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function groupDescription(family: string) {
  return groupDescriptions[family] || "Checks related to this rule family.";
}

function renderReportMarkdown(diagnostics: Diagnostic[], checkedFiles: string[], filesChecked: number, errors: number, warnings: number) {
  let output = "# Paper Linter Report\n\n";
  output += "## Summary\n\n";
  output += `checked ${filesChecked} file(s), ${errors} error(s), ${warnings} warning(s)\n\n`;
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
  if (bytes > MAX_UPLOAD_BYTES) throw new Error("input exceeds the 100 MB limit");
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
