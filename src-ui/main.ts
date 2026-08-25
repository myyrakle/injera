import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type Mode = "sequence" | "regex" | "convert";

type MessageKind = "idle" | "success" | "error";

type SequenceOptions = {
  prefix: string;
  start: number;
  padding: number | null;
};

type DirectoryEntry = {
  path: string;
  name: string;
};

type FileEntry = {
  path: string;
  name: string;
  size: number;
  is_archive: boolean;
  is_image: boolean;
};

type DirectoryListing = {
  path: string;
  parent: string | null;
  directories: DirectoryEntry[];
  files: FileEntry[];
};

type ArchivePreview = {
  entry_name: string;
  data_uri: string;
};

type RenameEntry = {
  source: string;
  target: string;
  source_name: string;
  target_name: string;
  selected: boolean;
};

type RenamePlan = {
  directory: string;
  entries: RenameEntry[];
};

type RenameReport = {
  renamed_count: number;
};

type TargetFormat = "keep" | "jpeg" | "png" | "webp";

type ConvertOptions = {
  format: TargetFormat;
  quality: number;
  suffix: string;
};

type ConvertKind = "archive" | "image";

type ConvertEntry = {
  kind: ConvertKind;
  source: string;
  source_name: string;
  output: string;
  output_name: string;
  selected: boolean;
  blocked: string | null;
};

type ConvertPlan = {
  entries: ConvertEntry[];
};

type ConvertProgress = {
  name: string;
  done: number;
  total: number;
};

type ConvertReport = {
  output: string;
  output_name: string;
  images_converted: number;
  images_skipped: number;
  images_kept: number;
  entries_copied: number;
  source_bytes: number;
  output_bytes: number;
};

/** One row of the plan table, whichever mode produced it. */
type PlanRow = {
  source: string;
  source_name: string;
  target: string;
  target_name: string;
  selected: boolean;
  blocked: string | null;
};

type State = {
  mode: Mode;
  listing: DirectoryListing | null;
  selected: Set<string>;
  plan: RenamePlan | null;
  conversion: ConvertPlan | null;
  busy: boolean;
  applied: boolean;
  message: string;
  messageKind: MessageKind;
};

const MAX_PADDING = 20;
const LAST_DIRECTORY_KEY = "injera:last-directory";

/**
 * The folder to reopen on launch. Browser storage can be unavailable, and the
 * stored folder can be gone, so every read is best-effort.
 */
function rememberedDirectory(): string | null {
  try {
    return localStorage.getItem(LAST_DIRECTORY_KEY);
  } catch {
    return null;
  }
}

function rememberDirectory(path: string) {
  try {
    localStorage.setItem(LAST_DIRECTORY_KEY, path);
  } catch {
    // A private window or blocked storage simply forgets the folder.
  }
}

const state: State = {
  mode: "sequence",
  listing: null,
  selected: new Set(),
  plan: null,
  conversion: null,
  busy: false,
  applied: false,
  message: "Loading",
  messageKind: "idle",
};

/** Archive path to its preview data URI, or `null` when it has none. */
const thumbnails = new Map<string, string | null>();
let thumbnailObserver: IntersectionObserver | null = null;

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("app root is missing");
}

app.innerHTML = `
  <main class="shell">
    <section class="toolbar" aria-label="Toolbar">
      <div class="brand">
        <span class="mark">
          <svg viewBox="0 0 48 48" role="img" aria-label="injera">
            <defs>
              <radialGradient id="crumb-mark" cx="42%" cy="38%" r="72%">
                <stop offset="0%" stop-color="#EFE6D0" />
                <stop offset="62%" stop-color="#DFD0AE" />
                <stop offset="100%" stop-color="#C9B48A" />
              </radialGradient>
            </defs>
            <circle cx="24" cy="24" r="22.56" fill="url(#crumb-mark)" stroke="#A98F63" stroke-width="1.4" />
            <g fill="#B99F73" fill-opacity="0.75">
              <ellipse cx="7.7" cy="17.6" rx="2.7" ry="2.4" />
              <ellipse cx="32.1" cy="29.4" rx="3.3" ry="2.9" />
              <ellipse cx="29.7" cy="9.4" rx="2.1" ry="1.9" />
              <ellipse cx="42.5" cy="21.9" rx="2.1" ry="1.8" />
              <ellipse cx="19.4" cy="23.2" rx="2.0" ry="1.8" />
              <ellipse cx="8.4" cy="30.1" rx="2.7" ry="2.4" />
              <ellipse cx="14.8" cy="40.1" rx="2.1" ry="1.9" />
              <ellipse cx="18.7" cy="7.4" rx="2.0" ry="1.8" />
              <ellipse cx="27.0" cy="40.0" rx="2.4" ry="2.1" />
            </g>
          </svg>
        </span>
        <div>
          <h1>injera</h1>
          <p>Batch rename</p>
        </div>
      </div>

      <div class="status" id="status" role="status">Loading</div>

      <button class="button primary" id="open-sheet" type="button">Rename or convert</button>
    </section>

    <section class="panel browser" aria-label="File browser">
      <div class="panel-head">
        <h2>Files</h2>
        <span id="selection-count">0 selected</span>
      </div>

      <div class="path-row">
        <button class="button ghost" id="up" type="button" title="Parent folder">Up</button>
        <button class="button ghost" id="home" type="button" title="Home folder">Home</button>
        <input id="path" readonly aria-label="Current folder" />
        <button class="button secondary" id="choose-directory" type="button">Choose</button>
      </div>

      <div class="path-row secondary-row">
        <button class="button ghost" id="select-all-files" type="button">Select all</button>
        <button class="button ghost" id="clear-selection" type="button">Clear</button>
        <span class="hint" id="folder-summary"></span>
      </div>

      <div class="browser-body" id="browser-body">
        <ul class="folder-list" id="folders"></ul>
        <table class="file-table">
          <tbody id="files"></tbody>
        </table>
      </div>
    </section>
  </main>

  <dialog class="sheet" id="sheet" aria-label="Rename and convert">
    <div class="sheet-head">
      <h2 id="panel-title">Rename</h2>
      <span id="preview-count">No preview</span>
      <button class="button ghost" id="sheet-close" type="button">Close</button>
    </div>

    <div class="sheet-body">
      <div class="mode-row" role="group" aria-label="Mode">
        <button class="segment" id="mode-sequence" type="button">Sequence</button>
        <button class="segment" id="mode-regex" type="button">Regex</button>
        <button class="segment" id="mode-convert" type="button">Convert</button>
      </div>

      <div class="fields sequence-controls" id="sequence-controls">
        <label class="field">
          <span>Prefix</span>
          <input id="prefix" placeholder="none" />
        </label>
        <label class="field">
          <span>Start number</span>
          <input id="start" inputmode="numeric" placeholder="1" />
        </label>
        <label class="field">
          <span>Padding</span>
          <input id="padding" inputmode="numeric" placeholder="auto" />
        </label>
      </div>

      <div class="fields regex-controls" id="regex-controls">
        <label class="field">
          <span>Pattern</span>
          <input id="pattern" placeholder="^IMG_(\\d+)\\.(jpg|png)$" />
        </label>
        <label class="field">
          <span>Replacement</span>
          <input id="replacement" placeholder="photo-$1.$2" />
        </label>
      </div>

      <div class="fields convert-controls" id="convert-controls">
        <label class="field">
          <span>Image format</span>
          <select id="format">
            <option value="keep">Keep original</option>
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
            <option value="webp">WebP</option>
          </select>
        </label>
        <label class="field">
          <span>Quality</span>
          <input id="quality" inputmode="numeric" value="80" />
        </label>
        <label class="field">
          <span>Suffix</span>
          <input id="suffix" value="-compressed" />
        </label>
      </div>

      <div class="table-wrap plan-wrap">
        <table>
          <thead>
            <tr>
              <th class="select-cell">
                <input type="checkbox" id="select-all" aria-label="Include every file" />
              </th>
              <th>Current</th>
              <th>New</th>
            </tr>
          </thead>
          <tbody id="rows"></tbody>
        </table>
      </div>
    </div>

    <div class="sheet-foot">
      <div class="progress" id="progress" hidden>
        <div class="progress-track">
          <div class="progress-fill" id="progress-fill"></div>
        </div>
        <span class="progress-label" id="progress-label"></span>
      </div>
      <button class="button danger" id="apply" type="button">Apply</button>
    </div>
  </dialog>
`;

function required<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);

  if (!element) {
    throw new Error(`${selector} is missing`);
  }

  return element;
}

const el = {
  status: required<HTMLDivElement>("#status"),
  path: required<HTMLInputElement>("#path"),
  up: required<HTMLButtonElement>("#up"),
  home: required<HTMLButtonElement>("#home"),
  chooseDirectory: required<HTMLButtonElement>("#choose-directory"),
  selectAllFiles: required<HTMLButtonElement>("#select-all-files"),
  clearSelection: required<HTMLButtonElement>("#clear-selection"),
  folderSummary: required<HTMLSpanElement>("#folder-summary"),
  selectionCount: required<HTMLSpanElement>("#selection-count"),
  browserBody: required<HTMLDivElement>("#browser-body"),
  folders: required<HTMLUListElement>("#folders"),
  files: required<HTMLTableSectionElement>("#files"),
  modeSequence: required<HTMLButtonElement>("#mode-sequence"),
  modeRegex: required<HTMLButtonElement>("#mode-regex"),
  modeConvert: required<HTMLButtonElement>("#mode-convert"),
  sequenceControls: required<HTMLElement>("#sequence-controls"),
  regexControls: required<HTMLElement>("#regex-controls"),
  convertControls: required<HTMLElement>("#convert-controls"),
  format: required<HTMLSelectElement>("#format"),
  quality: required<HTMLInputElement>("#quality"),
  suffix: required<HTMLInputElement>("#suffix"),
  prefix: required<HTMLInputElement>("#prefix"),
  start: required<HTMLInputElement>("#start"),
  padding: required<HTMLInputElement>("#padding"),
  pattern: required<HTMLInputElement>("#pattern"),
  replacement: required<HTMLInputElement>("#replacement"),
  apply: required<HTMLButtonElement>("#apply"),
  previewCount: required<HTMLSpanElement>("#preview-count"),
  panelTitle: required<HTMLHeadingElement>("#panel-title"),
  selectAll: required<HTMLInputElement>("#select-all"),
  rows: required<HTMLTableSectionElement>("#rows"),
  sheet: required<HTMLDialogElement>("#sheet"),
  openSheet: required<HTMLButtonElement>("#open-sheet"),
  sheetClose: required<HTMLButtonElement>("#sheet-close"),
  progress: required<HTMLDivElement>("#progress"),
  progressFill: required<HTMLDivElement>("#progress-fill"),
  progressLabel: required<HTMLSpanElement>("#progress-label"),
};

function setState(patch: Partial<State>) {
  const planChanged =
    ("plan" in patch && patch.plan !== state.plan) ||
    ("conversion" in patch && patch.conversion !== state.conversion) ||
    ("mode" in patch && patch.mode !== state.mode);
  Object.assign(state, patch);

  if (planChanged) {
    renderPlanRows();
  }

  update();
}

/** The plan table rows for the active mode. */
function planRows(): PlanRow[] {
  if (state.mode === "convert") {
    return (state.conversion?.entries ?? []).map((entry) => ({
      source: entry.source,
      source_name: entry.source_name,
      target: entry.output,
      target_name: entry.output_name,
      selected: entry.selected,
      blocked: entry.blocked,
    }));
  }

  return (state.plan?.entries ?? []).map((entry) => ({ ...entry, blocked: null }));
}

/** The entries backing `planRows`, so a checkbox can write its selection back. */
function planTargets(): Array<{ selected: boolean; blocked?: string | null }> {
  return state.mode === "convert"
    ? (state.conversion?.entries ?? [])
    : (state.plan?.entries ?? []);
}

function hasPlan(): boolean {
  return state.mode === "convert" ? state.conversion !== null : state.plan !== null;
}

/**
 * Files eligible for the active mode. Convert handles archives and images;
 * renaming handles anything.
 */
function eligibleFiles(): string[] {
  return (state.listing?.files ?? [])
    .filter((file) => state.selected.has(file.path))
    .filter((file) => state.mode !== "convert" || file.is_archive || file.is_image)
    .map((file) => file.path);
}

let previewTimer: ReturnType<typeof setTimeout> | undefined;
/** Bumped per request, so a slow plan cannot overwrite a newer one. */
let previewToken = 0;

/**
 * Re-plans once the inputs settle. The plan follows from the selection and the
 * settings, so there is nothing for the user to press.
 */
function schedulePreview() {
  setState({ plan: null, conversion: null, applied: false });
  clearTimeout(previewTimer);
  previewTimer = setTimeout(() => void refreshPlan(), 150);
}

function parseCount(value: string): number | null {
  const trimmed = value.trim();

  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  const parsed = Number(trimmed);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

/** Returns the sequence options, or `null` when an input is not usable yet. */
function sequenceOptions(): SequenceOptions | null {
  const prefix = el.prefix.value;

  if (prefix.includes("/") || prefix.includes("\\")) {
    return null;
  }

  const startRaw = el.start.value.trim();
  const start = startRaw === "" ? 1 : parseCount(startRaw);

  if (start === null) {
    return null;
  }

  const paddingRaw = el.padding.value.trim();

  if (paddingRaw === "") {
    return { prefix, start, padding: null };
  }

  const padding = parseCount(paddingRaw);

  if (padding === null || padding > MAX_PADDING) {
    return null;
  }

  return { prefix, start, padding };
}

/** Returns the conversion options, or `null` when an input is not usable yet. */
function convertOptions(): ConvertOptions | null {
  const quality = parseCount(el.quality.value);

  if (quality === null || quality < 1 || quality > 100) {
    return null;
  }

  const suffix = el.suffix.value;

  if (suffix.length === 0 || suffix.includes("/") || suffix.includes("\\")) {
    return null;
  }

  return { format: el.format.value as TargetFormat, quality, suffix };
}

function planSelectedCount(): number {
  return planTargets().filter((entry) => entry.selected).length;
}

function update() {
  const files = state.listing?.files ?? [];

  el.status.textContent = state.message;
  el.status.className = `status status-${state.messageKind}`;
  el.path.value = state.listing?.path ?? "";
  el.path.title = state.listing?.path ?? "";
  // Long paths matter at the tail, so keep the current folder in view.
  el.path.scrollLeft = el.path.scrollWidth;

  el.up.disabled = state.busy || !state.listing?.parent;
  el.home.disabled = state.busy;
  el.chooseDirectory.disabled = state.busy;
  el.selectAllFiles.disabled = state.busy || files.length === 0;
  el.openSheet.disabled = state.busy || state.selected.size === 0;
  el.openSheet.textContent =
    state.selected.size > 0 ? `Rename or convert (${state.selected.size})` : "Rename or convert";
  el.sheetClose.disabled = state.busy;
  el.clearSelection.disabled = state.busy || state.selected.size === 0;

  el.modeSequence.classList.toggle("active", state.mode === "sequence");
  el.modeRegex.classList.toggle("active", state.mode === "regex");
  el.modeConvert.classList.toggle("active", state.mode === "convert");
  el.sequenceControls.hidden = state.mode !== "sequence";
  el.regexControls.hidden = state.mode !== "regex";
  el.convertControls.hidden = state.mode !== "convert";
  // PNG is lossless, so quality has nothing to act on there.
  el.quality.disabled = el.format.value === "png";
  el.apply.textContent = state.mode === "convert" ? "Convert" : "Apply";
  el.panelTitle.textContent = state.mode === "convert" ? "Convert" : "Rename";

  el.selectionCount.textContent = `${state.selected.size} selected`;
  el.folderSummary.textContent = state.listing
    ? `${state.listing.directories.length} folders · ${files.length} files`
    : "";


  updatePlanUi();
}

function updatePlanUi() {
  const entries = planTargets();
  const selected = planSelectedCount();

  el.previewCount.textContent = hasPlan()
    ? `${selected} of ${entries.length} selected`
    : "No preview";
  el.selectAll.disabled = entries.length === 0 || state.busy;
  el.selectAll.checked = entries.length > 0 && selected === entries.length;
  el.selectAll.indeterminate = selected > 0 && selected < entries.length;
  el.apply.disabled = !hasPlan() || state.busy || state.applied || selected === 0;
}

function formatSize(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

function renderBrowser() {
  const listing = state.listing;

  el.folders.innerHTML = (listing?.directories ?? [])
    .map(
      (entry) => `
        <li>
          <button class="folder" type="button" data-folder="${escapeAttribute(entry.path)}">
            <span class="folder-icon" aria-hidden="true"></span>
            <span class="folder-name">${escapeHtml(entry.name)}</span>
          </button>
        </li>
      `,
    )
    .join("");

  const files = listing?.files ?? [];

  if (files.length === 0) {
    el.files.innerHTML = `<tr><td class="empty" colspan="3">${
      listing ? "No files in this folder." : "Loading folder."
    }</td></tr>`;
  } else {
    el.files.innerHTML = files
      .map(
        (file) => `
          <tr data-file-row="${escapeAttribute(file.path)}">
            <td class="select-cell">
              <input
                type="checkbox"
                data-file="${escapeAttribute(file.path)}"
                ${state.selected.has(file.path) ? "checked" : ""}
                aria-label="Select ${escapeAttribute(file.name)}"
              />
            </td>
            <td class="thumb-cell">${
              file.is_archive
                ? `<span class="thumb" data-archive="${escapeAttribute(file.path)}"></span>`
                : `<span class="thumb thumb-plain" aria-hidden="true"></span>`
            }</td>
            <td class="file-cell">
              <span class="file-name" title="${escapeAttribute(file.path)}">${escapeHtml(file.name)}</span>
              <span class="file-meta">${formatSize(file.size)}${
                file.is_archive ? " · archive" : file.is_image ? " · image" : ""
              }</span>
            </td>
          </tr>
        `,
      )
      .join("");
  }

  observeThumbnails();
}

function observeThumbnails() {
  thumbnailObserver?.disconnect();
  thumbnailObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) {
          continue;
        }

        const cell = entry.target as HTMLElement;
        thumbnailObserver?.unobserve(cell);
        void loadThumbnail(cell);
      }
    },
    { root: el.browserBody, rootMargin: "300px" },
  );

  for (const cell of el.files.querySelectorAll<HTMLElement>("[data-archive]")) {
    thumbnailObserver.observe(cell);
  }
}

async function loadThumbnail(cell: HTMLElement) {
  const path = cell.dataset.archive;

  if (!path) {
    return;
  }

  if (thumbnails.has(path)) {
    paintThumbnail(cell, thumbnails.get(path) ?? null);
    return;
  }

  cell.classList.add("thumb-loading");

  try {
    const preview = await invoke<ArchivePreview | null>("archive_preview", { path });
    thumbnails.set(path, preview?.data_uri ?? null);
    paintThumbnail(cell, preview?.data_uri ?? null);
  } catch {
    // A damaged or unsupported archive simply has no preview.
    thumbnails.set(path, null);
    paintThumbnail(cell, null);
  }
}

function paintThumbnail(cell: HTMLElement, dataUri: string | null) {
  cell.classList.remove("thumb-loading");

  if (dataUri) {
    cell.innerHTML = `<img src="${dataUri}" alt="" />`;
  } else {
    cell.classList.add("thumb-plain");
  }
}

/** Why the plan is empty, so the table can say so instead of sitting blank. */
function planHint(): string {
  if (eligibleFiles().length === 0) {
    return state.mode === "convert"
      ? "Tick the archives or images to convert."
      : "Tick the files to rename.";
  }

  if (state.mode === "regex" && el.pattern.value.length === 0) {
    return "Enter a pattern.";
  }

  if (state.mode === "sequence" && sequenceOptions() === null) {
    return "Check the sequence settings.";
  }

  if (state.mode === "convert" && convertOptions() === null) {
    return "Check the conversion settings.";
  }

  return "Working out the plan.";
}

function renderPlanRows() {
  const entries = planRows();

  if (entries.length === 0) {
    el.rows.innerHTML = `<tr><td class="empty" colspan="3">${escapeHtml(planHint())}</td></tr>`;
    return;
  }

  el.rows.innerHTML = entries
    .map(
      (entry, index) => `
        <tr class="${entry.blocked ? "blocked" : ""}">
          <td class="select-cell">
            <input
              type="checkbox"
              data-index="${index}"
              ${entry.selected ? "checked" : ""}
              ${entry.blocked ? "disabled" : ""}
              aria-label="Include ${escapeAttribute(entry.source_name)}"
            />
          </td>
          <td title="${escapeAttribute(entry.source)}">${escapeHtml(entry.source_name)}</td>
          <td title="${escapeAttribute(entry.target)}">${escapeHtml(entry.target_name)}${
            entry.blocked ? `<span class="row-note">${escapeHtml(entry.blocked)}</span>` : ""
          }</td>
        </tr>
      `,
    )
    .join("");
}

/**
 * Reloads a folder. `keepSelection` carries the current selection across the
 * reload, for actions that leave their source files in place.
 */
async function openDirectory(path: string | null, message = "Ready", keepSelection = false) {
  setState({ busy: true, message: "Loading folder", messageKind: "idle" });
  const previous = keepSelection ? new Set(state.selected) : null;

  try {
    const listing = await invoke<DirectoryListing>("list_directory", { path });
    state.selected.clear();

    if (previous) {
      // Only keep what is still there; a source can vanish between runs.
      for (const file of listing.files) {
        if (previous.has(file.path)) {
          state.selected.add(file.path);
        }
      }
    }

    rememberDirectory(listing.path);
    setState({
      listing,
      plan: null,
      conversion: null,
      applied: false,
      busy: false,
      message,
      messageKind: "idle",
    });
    renderBrowser();
    el.browserBody.scrollTop = 0;
    void refreshPlan();
  } catch (error) {
    if (path !== null) {
      // A remembered folder can be renamed or unplugged; fall back to home.
      await openDirectory(null, message, keepSelection);
      return;
    }

    setState({ busy: false, message: String(error), messageKind: "error" });
  }
}

async function chooseDirectory() {
  try {
    const selected = await open({ directory: true, multiple: false, title: "Choose folder" });

    if (typeof selected === "string") {
      await openDirectory(selected, "Folder opened");
    }
  } catch (error) {
    setState({ message: String(error), messageKind: "error" });
  }
}

async function refreshPlan() {
  const files = eligibleFiles();
  const settingsReady =
    state.mode === "sequence"
      ? sequenceOptions() !== null
      : state.mode === "regex"
        ? el.pattern.value.length > 0
        : convertOptions() !== null;

  if (state.busy || files.length === 0 || !settingsReady) {
    update();
    return;
  }

  const token = ++previewToken;
  // A fresh plan clears a stale error, but must not talk over a result the
  // user just earned, such as "3 files converted".
  const cleared: Partial<State> =
    state.messageKind === "error" ? { message: "Ready", messageKind: "idle" } : {};

  try {
    if (state.mode === "convert") {
      const conversion = await invoke<ConvertPlan>("preview_conversion", {
        files,
        options: convertOptions(),
      });

      if (token === previewToken) {
        setState({ conversion, applied: false, ...cleared });
      }

      return;
    }

    const plan =
      state.mode === "sequence"
        ? await invoke<RenamePlan>("preview_sequence", { files, options: sequenceOptions() })
        : await invoke<RenamePlan>("preview_regex", {
            files,
            pattern: el.pattern.value,
            replacement: el.replacement.value,
          });

    if (token === previewToken) {
      setState({ plan, applied: false, ...cleared });
    }
  } catch (error) {
    if (token === previewToken) {
      setState({ plan: null, conversion: null, message: String(error), messageKind: "error" });
    }
  }
}

/** Draws the bar, or hides it when `percent` is null. */
function showProgress(percent: number | null, label = "") {
  el.progress.hidden = percent === null;

  if (percent !== null) {
    el.progressFill.style.width = `${Math.max(0, Math.min(100, percent))}%`;
    el.progressLabel.textContent = label;
  }
}

async function runConversion() {
  const entries = (state.conversion?.entries ?? []).filter((entry) => entry.selected);
  const options = convertOptions();

  if (entries.length === 0 || !options) {
    return;
  }

  setState({ busy: true, message: "Converting", messageKind: "idle" });

  let done = 0;
  let saved = 0;

  // Progress inside the running entry, so the bar moves during a long archive.
  const onProgress = (progress: ConvertProgress) => {
    const within = progress.total > 0 ? progress.done / progress.total : 0;
    const percent = ((done + within) / entries.length) * 100;
    const counter = entries.length > 1 ? ` (${done + 1}/${entries.length})` : "";
    showProgress(percent, `${Math.floor(percent)}% · ${progress.name}${counter}`);
    el.status.textContent = `Converting ${progress.name} · ${Math.floor(percent)}%`;
  };

  const stop = await listen<ConvertProgress>("convert://progress", (event) =>
    onProgress(event.payload),
  );

  try {
    showProgress(0, "0%");

    for (const entry of entries) {
      const report = await invoke<ConvertReport>("convert_entry", { entry, options });
      done += 1;
      saved += report.source_bytes - report.output_bytes;
      showProgress((done / entries.length) * 100, `${Math.round((done / entries.length) * 100)}%`);
    }

    const change =
      saved >= 0 ? `${formatSize(saved)} saved` : `${formatSize(-saved)} larger`;
    const noun = done === 1 ? "file" : "files";
    setState({ busy: false, applied: true });
    // Conversion leaves its sources alone, so the selection is still valid and
    // the run can be repeated with different settings.
    await openDirectory(state.listing?.path ?? null, "Ready", true);
    setState({
      message: `${done} ${noun} converted · ${change}`,
      messageKind: "success",
    });
  } catch (error) {
    setState({
      busy: false,
      message: done > 0 ? `${done} converted, then failed: ${error}` : String(error),
      messageKind: "error",
    });
  } finally {
    // Clear the bar first, so a failing unlisten cannot strand it on screen.
    showProgress(null);
    stop();
  }
}

async function applyRename() {
  if (state.mode === "convert") {
    await runConversion();
    return;
  }

  if (!state.plan || state.busy || state.applied || planSelectedCount() === 0) {
    return;
  }

  setState({ busy: true, message: "Renaming", messageKind: "idle" });

  try {
    const report = await invoke<RenameReport>("apply_rename", { plan: state.plan });
    const directory = state.listing?.path ?? null;
    setState({ busy: false, applied: true });
    el.sheet.close();
    await openDirectory(directory, `${report.renamed_count} files renamed`);
    setState({ message: `${report.renamed_count} files renamed`, messageKind: "success" });
  } catch (error) {
    setState({ busy: false, message: String(error), messageKind: "error" });
  }
}

function changeMode(mode: Mode) {
  setState({
    mode,
    plan: null,
    conversion: null,
    applied: false,
    message: "Ready",
    messageKind: "idle",
  });
  void refreshPlan();
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function escapeAttribute(value: string) {
  return escapeHtml(value);
}

function openSheet() {
  if (state.selected.size === 0 || el.sheet.open) {
    return;
  }

  el.sheet.showModal();
  void refreshPlan();
}

function closeSheet() {
  if (!state.busy) {
    el.sheet.close();
  }
}

el.openSheet.addEventListener("click", openSheet);
el.sheetClose.addEventListener("click", closeSheet);
// Escape reaches the dialog directly, so block it mid-run.
el.sheet.addEventListener("cancel", (event) => {
  if (state.busy) {
    event.preventDefault();
  }
});

el.up.addEventListener("click", () => {
  if (state.listing?.parent) {
    void openDirectory(state.listing.parent);
  }
});
el.home.addEventListener("click", () => void openDirectory(null));
el.chooseDirectory.addEventListener("click", chooseDirectory);

el.folders.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest<HTMLElement>("[data-folder]");

  if (button?.dataset.folder && !state.busy) {
    void openDirectory(button.dataset.folder);
  }
});

el.files.addEventListener("change", (event) => {
  const checkbox = event.target;

  if (!(checkbox instanceof HTMLInputElement) || !checkbox.dataset.file) {
    return;
  }

  if (checkbox.checked) {
    state.selected.add(checkbox.dataset.file);
  } else {
    state.selected.delete(checkbox.dataset.file);
  }

  schedulePreview();
});

el.selectAllFiles.addEventListener("click", () => {
  for (const file of state.listing?.files ?? []) {
    state.selected.add(file.path);
  }

  syncFileCheckboxes();
  schedulePreview();
});

el.clearSelection.addEventListener("click", () => {
  state.selected.clear();
  syncFileCheckboxes();
  schedulePreview();
});

function syncFileCheckboxes() {
  for (const checkbox of el.files.querySelectorAll<HTMLInputElement>("input[data-file]")) {
    checkbox.checked = state.selected.has(checkbox.dataset.file ?? "");
  }
}

el.modeSequence.addEventListener("click", () => changeMode("sequence"));
el.modeRegex.addEventListener("click", () => changeMode("regex"));
el.modeConvert.addEventListener("click", () => changeMode("convert"));
el.apply.addEventListener("click", applyRename);

for (const input of [
  el.prefix,
  el.start,
  el.padding,
  el.pattern,
  el.replacement,
  el.quality,
  el.suffix,
]) {
  input.addEventListener("input", schedulePreview);
}

el.format.addEventListener("change", schedulePreview);

el.selectAll.addEventListener("change", () => {
  for (const entry of planTargets()) {
    if (!entry.blocked) {
      entry.selected = el.selectAll.checked;
    }
  }

  for (const checkbox of el.rows.querySelectorAll<HTMLInputElement>("input[data-index]")) {
    if (!checkbox.disabled) {
      checkbox.checked = el.selectAll.checked;
    }
  }

  updatePlanUi();
});

el.rows.addEventListener("change", (event) => {
  const checkbox = event.target;

  if (!(checkbox instanceof HTMLInputElement) || checkbox.dataset.index === undefined) {
    return;
  }

  const entry = planTargets()[Number(checkbox.dataset.index)];

  if (entry && !entry.blocked) {
    entry.selected = checkbox.checked;
    updatePlanUi();
  }
});

renderBrowser();
renderPlanRows();
update();
void openDirectory(rememberedDirectory());
