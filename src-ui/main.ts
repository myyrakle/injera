import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type Mode = "sequence" | "regex";

type RenameEntry = {
  source: string;
  target: string;
  source_name: string;
  target_name: string;
};

type RenamePlan = {
  directory: string;
  entries: RenameEntry[];
};

type RenameReport = {
  renamed_count: number;
};

type State = {
  mode: Mode;
  directory: string;
  pattern: string;
  replacement: string;
  plan: RenamePlan | null;
  busy: boolean;
  applied: boolean;
  message: string;
  messageKind: "idle" | "success" | "error";
};

const state: State = {
  mode: "sequence",
  directory: "",
  pattern: "",
  replacement: "",
  plan: null,
  busy: false,
  applied: false,
  message: "Ready",
  messageKind: "idle",
};

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("app root is missing");
}

const root = app;

function setState(patch: Partial<State>) {
  Object.assign(state, patch);
  render();
}

async function chooseDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Choose folder",
  });

  if (typeof selected === "string") {
    setState({
      directory: selected,
      plan: null,
      applied: false,
      message: "Directory selected",
      messageKind: "idle",
    });
  }
}

async function previewRename() {
  if (!state.directory || state.busy) {
    return;
  }

  setState({ busy: true, plan: null, applied: false, message: "Previewing", messageKind: "idle" });

  try {
    const plan =
      state.mode === "sequence"
        ? await invoke<RenamePlan>("preview_sequence", { directory: state.directory })
        : await invoke<RenamePlan>("preview_regex", {
            directory: state.directory,
            pattern: state.pattern,
            replacement: state.replacement,
          });

    setState({
      plan,
      busy: false,
      message: `${plan.entries.length} files ready`,
      messageKind: "success",
    });
  } catch (error) {
    setState({
      busy: false,
      message: String(error),
      messageKind: "error",
    });
  }
}

async function applyRename() {
  if (!state.plan || state.busy || state.applied) {
    return;
  }

  setState({ busy: true, message: "Renaming", messageKind: "idle" });

  try {
    const report = await invoke<RenameReport>("apply_rename", { plan: state.plan });
    setState({
      busy: false,
      applied: true,
      message: `${report.renamed_count} files renamed`,
      messageKind: "success",
    });
  } catch (error) {
    setState({
      busy: false,
      message: String(error),
      messageKind: "error",
    });
  }
}

function changeMode(mode: Mode) {
  setState({
    mode,
    plan: null,
    applied: false,
    message: "Ready",
    messageKind: "idle",
  });
}

function render() {
  const canPreview =
    Boolean(state.directory) && !state.busy && (state.mode === "sequence" || state.pattern.length > 0);
  const canApply = Boolean(state.plan) && !state.busy && !state.applied;

  root.innerHTML = `
    <main class="shell">
      <section class="toolbar" aria-label="Rename controls">
        <div class="brand">
          <span class="mark">I</span>
          <div>
            <h1>injera</h1>
            <p>Batch rename</p>
          </div>
        </div>

        <div class="status status-${state.messageKind}" role="status">${escapeHtml(state.message)}</div>
      </section>

      <section class="controls">
        <label class="field field-grow">
          <span>Directory</span>
          <input readonly value="${escapeAttribute(state.directory)}" placeholder="No folder selected" />
        </label>
        <button class="button secondary" id="choose-directory" type="button" ${state.busy ? "disabled" : ""}>
          Choose
        </button>
      </section>

      <section class="mode-row" aria-label="Rename mode">
        <button class="segment ${state.mode === "sequence" ? "active" : ""}" id="mode-sequence" type="button">
          Sequence
        </button>
        <button class="segment ${state.mode === "regex" ? "active" : ""}" id="mode-regex" type="button">
          Regex
        </button>
      </section>

      ${
        state.mode === "regex"
          ? `<section class="controls regex-controls">
              <label class="field">
                <span>Pattern</span>
                <input id="pattern" value="${escapeAttribute(state.pattern)}" placeholder="^IMG_(\\d+)\\.(jpg|png)$" />
              </label>
              <label class="field">
                <span>Replacement</span>
                <input id="replacement" value="${escapeAttribute(state.replacement)}" placeholder="photo-$1.$2" />
              </label>
            </section>`
          : ""
      }

      <section class="actions">
        <button class="button primary" id="preview" type="button" ${canPreview ? "" : "disabled"}>
          Preview
        </button>
        <button class="button danger" id="apply" type="button" ${canApply ? "" : "disabled"}>
          Apply
        </button>
      </section>

      <section class="preview" aria-label="Rename preview">
        <div class="preview-head">
          <h2>Preview</h2>
          <span>${state.plan ? `${state.plan.entries.length} files` : "No preview"}</span>
        </div>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Current</th>
                <th>New</th>
              </tr>
            </thead>
            <tbody>
              ${renderRows(state.plan)}
            </tbody>
          </table>
        </div>
      </section>
    </main>
  `;

  document.querySelector("#choose-directory")?.addEventListener("click", chooseDirectory);
  document.querySelector("#mode-sequence")?.addEventListener("click", () => changeMode("sequence"));
  document.querySelector("#mode-regex")?.addEventListener("click", () => changeMode("regex"));
  document.querySelector("#preview")?.addEventListener("click", previewRename);
  document.querySelector("#apply")?.addEventListener("click", applyRename);
  document.querySelector<HTMLInputElement>("#pattern")?.addEventListener("input", (event) => {
    const input = event.currentTarget as HTMLInputElement;
    setState({ pattern: input.value, plan: null, applied: false });
  });
  document.querySelector<HTMLInputElement>("#replacement")?.addEventListener("input", (event) => {
    const input = event.currentTarget as HTMLInputElement;
    setState({ replacement: input.value, plan: null, applied: false });
  });
}

function renderRows(plan: RenamePlan | null) {
  if (!plan || plan.entries.length === 0) {
    return `<tr><td class="empty" colspan="2">Select a folder and preview changes.</td></tr>`;
  }

  return plan.entries
    .map(
      (entry) => `
        <tr>
          <td title="${escapeAttribute(entry.source)}">${escapeHtml(entry.source_name)}</td>
          <td title="${escapeAttribute(entry.target)}">${escapeHtml(entry.target_name)}</td>
        </tr>
      `,
    )
    .join("");
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

render();
