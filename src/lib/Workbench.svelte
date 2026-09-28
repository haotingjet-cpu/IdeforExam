<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, highlightActiveLineGutter, keymap, lineNumbers } from "@codemirror/view";
  import { bracketMatching, defaultHighlightStyle, indentOnInput, indentUnit, syntaxHighlighting, syntaxTree } from "@codemirror/language";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { cpp } from "@codemirror/lang-cpp";
  import { searchKeymap } from "@codemirror/search";
  import localforage from "localforage";
  import { LanguageServerClient, languageServerWithTransport, type Transport } from "codemirror-languageserver";
  import { CircleCheck, CirclePlus, CircleX, Clock3, Code2, Copy, FileCode2, FolderOpen, FolderPlus, Minus, Play, Save, Search, Settings2, Square, Terminal, Trash2, X } from "lucide-svelte";

  type ResultStatus = "AC" | "WA" | "RE" | "TLE";
  type ConsoleTab = "output" | "build" | "diff";
  type ResizeKind = "sidebar" | "main" | "console";
  interface TestCase {
    id: string;
    name: string;
    input: string;
    expectedOutput: string;
    status?: ResultStatus;
    actualOutput?: string;
    stderr?: string;
    executionTimeMs?: number;
    firstDifference?: number | null;
  }
  interface CompileResult { success: boolean; output: string; executablePath: string | null }
  interface RunResult { stdout: string; stderr: string; executionTimeMs: number; exitCode: number | null; timedOut: boolean; cancelled: boolean }
  interface CompareResult { accepted: boolean; firstDifference: number | null }
  interface LspSessionInfo {
    sessionId: string;
    eventName: string;
    rootUri: string;
    documentUri: string;
    clangdPath: string;
  }
  interface ToolchainInfo { gxxPath: string; clangdPath: string; extracted: boolean }

  class TauriLspTransport implements Transport {
    private messageHandler: ((message: string) => void) | undefined;
    private closeHandler: (() => void) | undefined;
    private errorHandler: ((error: Error) => void) | undefined;
    private unlisten: UnlistenFn | undefined;
    private sessionId: string;
    private sendQueue: Promise<void> = Promise.resolve();

    constructor(sessionId: string) { this.sessionId = sessionId; }

    setUnlisten(unlisten: UnlistenFn) { this.unlisten = unlisten; }
    send(message: string) {
      this.sendQueue = this.sendQueue
        .then(() => invoke<void>("send_clangd", { sessionId: this.sessionId, message }))
        .catch((error) => { this.errorHandler?.(new Error(String(error))); });
    }
    onMessage(callback: (message: string) => void) { this.messageHandler = callback; }
    onClose(callback: () => void) { this.closeHandler = callback; }
    onError(callback: (error: Error) => void) { this.errorHandler = callback; }
    receive(message: string) {
      if (message === "__clangd_closed__") this.closeHandler?.();
      else if (message.startsWith("__clangd_error__:")) this.errorHandler?.(new Error(message));
      else this.messageHandler?.(message);
    }
    close() {
      this.unlisten?.();
      this.unlisten = undefined;
      this.sendQueue = this.sendQueue
        .then(() => invoke<void>("stop_clangd", { sessionId: this.sessionId }))
        .catch(() => {});
      this.closeHandler?.();
    }
  }

  //  cin.tie(nullptr);
  //  ios_base::sync_with_stdio(false);
  const starterCode = `#include <bits/stdc++.h>
using namespace std;

int main() {
    int value;

    cin >> value;
    cout << value;

    return 0;
}`;
  const storageKey = "ideforexam.test-cases.v1";
  const layoutStorageKey = "ideforexam.layout.v1";
  let editorElement: HTMLDivElement;
  let editorPanelElement: HTMLElement;
  let workbenchElement: HTMLElement;
  let editorView: EditorView | undefined;
  const lspCompartment = new Compartment();
  const editableCompartment = new Compartment();
  let lspTransport: TauriLspTransport | undefined;
  let languageClient: LanguageServerClient | undefined;
  let lspSession: LspSessionInfo | undefined;
  let clangdStatus = $state("等待開啟 C++ 檔案");
  let toolchainsReady = $state(false);
  let toolchainStatus = $state("正在檢查 C++ 工具鏈...");
  let toolchainInitialization: Promise<ToolchainInfo>;
  let source = $state("");
  let projectPath = $state("");
  let filePaths = $state<string[]>([]);
  let activePath = $state("");
  let hasFile = $derived(activePath !== "");
  let dirty = $state(false);
  let testCases = $state<TestCase[]>([{ id: "sample", name: "輸入輸出", input: "5\n", expectedOutput: "5\n" }]);
  let activeTestId = $state("sample");
  let activeTest = $derived(testCases.find((testCase) => testCase.id === activeTestId));

  interface DiffSegment { text: string; bad?: boolean; missing?: string }
  interface DiffRow { kind: "same" | "changed" | "added" | "missing"; segments: DiffSegment[]; line: number | null }
  interface DiffView { rows: DiffRow[]; hasDiff: boolean; diffCount: number; firstNote: string; lineCount: number }
  interface DiffOp { op: "eq" | "del" | "ins"; a: number; b: number }

  function splitDiffLines(text: string) {
    if (text === "") return [] as string[];
    const lines = text.replace(/\r\n?/g, "\n").split("\n");
    if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
    return lines;
  }
  function numberedLines(text: string) { return splitDiffLines(text); }
  function gutterStyle(count: number) { return "--gw:" + Math.max(String(count).length, 2) + "ch"; }
  // 通用 LCS diff；輸入太大時退回逐位置比較，避免卡住 UI
  function diffOps<T>(a: T[], b: T[]): DiffOp[] {
    const n = a.length, m = b.length;
    const ops: DiffOp[] = [];
    if ((n + 1) * (m + 1) > 4_000_000) {
      for (let i = 0; i < Math.max(n, m); i++) {
        if (i < n && i < m && a[i] === b[i]) ops.push({ op: "eq", a: i, b: i });
        else { if (i < n) ops.push({ op: "del", a: i, b: Math.min(i, m) }); if (i < m) ops.push({ op: "ins", a: Math.min(i, n), b: i }); }
      }
      return ops;
    }
    const w = m + 1;
    const table = new Uint32Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) {
      table[i * w + j] = a[i] === b[j] ? table[(i + 1) * w + j + 1] + 1 : Math.max(table[(i + 1) * w + j], table[i * w + j + 1]);
    }
    let i = 0, j = 0;
    while (i < n && j < m) {
      if (a[i] === b[j]) { ops.push({ op: "eq", a: i, b: j }); i++; j++; }
      else if (table[(i + 1) * w + j] >= table[i * w + j + 1]) { ops.push({ op: "del", a: i, b: j }); i++; }
      else { ops.push({ op: "ins", a: i, b: j }); j++; }
    }
    while (i < n) ops.push({ op: "del", a: i++, b: j });
    while (j < m) ops.push({ op: "ins", a: i, b: j++ });
    return ops;
  }
  // 單行字元層級 diff：ACTUAL 多出／不同的字元標 bad，少掉的字元以 ‸ 標記
  function diffChars(expected: string, actual: string): DiffSegment[] {
    const e = Array.from(expected), a = Array.from(actual);
    const segments: DiffSegment[] = [];
    let pending: string[] = [];
    const push = (text: string, bad: boolean) => {
      const last = segments[segments.length - 1];
      if (last && !last.missing && !!last.bad === bad) last.text += text;
      else segments.push({ text, bad });
    };
    const flush = () => { if (pending.length) { segments.push({ text: "", missing: pending.join("") }); pending = []; } };
    for (const op of diffOps(e, a)) {
      if (op.op === "del") pending.push(e[op.a]);
      else if (op.op === "ins") { if (pending.length) pending.shift(); push(a[op.b], true); } // 有配對到的刪除視為「替換」，不另外標缺字
      else { flush(); push(a[op.b], false); }
    }
    flush();
    return segments;
  }
  function buildDiff(expected: string, actual: string): DiffView {
    const e = splitDiffLines(expected), a = splitDiffLines(actual);
    const ops = diffOps(e, a);
    const rows: DiffRow[] = [];
    let actualLines = 0;
    let firstNote = "";
    let k = 0;
    while (k < ops.length) {
      if (ops[k].op === "eq") { actualLines++; rows.push({ kind: "same", segments: [{ text: a[ops[k].b] }], line: actualLines }); k++; continue; }
      const dels: number[] = [], inss: number[] = [];
      while (k < ops.length && ops[k].op !== "eq") { if (ops[k].op === "del") dels.push(ops[k].a); else inss.push(ops[k].b); k++; }
      const pairs = Math.min(dels.length, inss.length);
      for (let p = 0; p < pairs; p++) {
        const segments = diffChars(e[dels[p]], a[inss[p]]);
        actualLines++;
        if (!firstNote) {
          let col = 0;
          for (const seg of segments) { if (seg.bad || seg.missing) break; col += Array.from(seg.text).length; }
          firstNote = `第一個差異位於第 ${actualLines} 行第 ${col + 1} 個字元`;
        }
        rows.push({ kind: "changed", segments, line: actualLines });
      }
      for (let p = pairs; p < inss.length; p++) {
        actualLines++;
        if (!firstNote) firstNote = `第 ${actualLines} 行起為多餘的輸出`;
        rows.push({ kind: "added", segments: [{ text: a[inss[p]] === "" ? " " : a[inss[p]], bad: true }], line: actualLines });
      }
      for (let p = pairs; p < dels.length; p++) {
        if (!firstNote) firstNote = `第 ${actualLines + 1} 行起輸出不足`;
        rows.push({ kind: "missing", segments: [{ text: e[dels[p]] }], line: null });
      }
    }
    const diffCount = rows.filter((row) => row.kind !== "same").length;
    return { rows, hasDiff: diffCount > 0, diffCount, firstNote, lineCount: actualLines };
  }
  let timeoutMs = $state(2000);
  let busy = $state("");
  let activeRunId = $state("");
  let cancelRequested = $state(false);
  let compilerOutput = $state("");
  let stdout = $state("");
  let stderr = $state("");
  let consoleTab = $state<ConsoleTab>("output");
  let showDiffMarks = $state(false);
  let notice = $state("準備就緒");
  let noticeTone = $state("neutral");
  let showNewProject = $state(false);
  let projectName = $state("");
  let sidebarWidth = $state(205);
  let testWidth = $state(280);
  let consoleHeight = $state(190);
  let editorHeight = $state(330);
  let testHeight = $state(210);
  let sidebarHeight = $state(130);
  let activeResize = $state<{ kind: ResizeKind; pointerId: number } | null>(null);
  let diffCols = $state([1 / 3, 1 / 3, 1 / 3]);
  let diffResize = $state<{ index: number; pointerId: number } | null>(null);
  let diffColumnsElement = $state<HTMLElement | undefined>();
  const minDiffCol = 0.12;
  let showTestManager = $state(false);
  let appWindow: ReturnType<typeof getCurrentWindow> | undefined;
  let isMaximized = $state(false);
  let viewportMode = $state<"desktop" | "stacked" | "mobile">("desktop");

  onMount(() => {
    const updateViewportMode = () => {
      viewportMode = window.innerWidth <= 740 ? "mobile" : window.innerWidth <= 980 ? "stacked" : "desktop";
    };
    updateViewportMode();
    window.addEventListener("resize", updateViewportMode);
    let disposed = false;
    let unlistenResize: UnlistenFn | undefined;
    try {
      appWindow = getCurrentWindow();
      const syncMaximized = () => { void appWindow?.isMaximized().then((value) => { isMaximized = value; }).catch(() => {}); };
      syncMaximized();
      void appWindow.onResized(syncMaximized).then((unlisten) => {
        if (disposed) unlisten();
        else unlistenResize = unlisten;
      }).catch(() => {});
    } catch { /* 非 Tauri 環境（例如純瀏覽器預覽）時不顯示視窗控制功能。 */ }
    toolchainInitialization = invoke<ToolchainInfo>("ensure_toolchains");
    void toolchainInitialization.then((toolchains) => {
      toolchainsReady = true;
      toolchainStatus = toolchains.extracted ? "C++ 工具鏈已安裝" : "G++ · clangd 就緒";
    }).catch((error) => {
      toolchainStatus = `工具鏈準備失敗：${String(error)}`;
      setNotice(toolchainStatus, "error");
    });

    try {
      const layout = JSON.parse(localStorage.getItem(layoutStorageKey) ?? "{}") as Record<string, unknown>;
      if (typeof layout.sidebarWidth === "number") sidebarWidth = clamp(layout.sidebarWidth, 155, 360);
      if (typeof layout.testWidth === "number") testWidth = clamp(layout.testWidth, 235, 480);
      if (typeof layout.consoleHeight === "number") consoleHeight = clamp(layout.consoleHeight, 150, 420);
      if (typeof layout.editorHeight === "number") editorHeight = clamp(layout.editorHeight, 240, 560);
      if (typeof layout.testHeight === "number") testHeight = clamp(layout.testHeight, 180, 420);
      if (typeof layout.sidebarHeight === "number") sidebarHeight = clamp(layout.sidebarHeight, 100, 240);
      if (Array.isArray(layout.diffCols) && layout.diffCols.length === 3 && layout.diffCols.every((value) => typeof value === "number" && value >= minDiffCol && value <= 1)) {
        const total = layout.diffCols.reduce((sum: number, value: number) => sum + value, 0);
        diffCols = layout.diffCols.map((value: number) => value / total);
      }
    } catch {
      localStorage.removeItem(layoutStorageKey);
    }

    editorView = new EditorView({
      state: EditorState.create({
        doc: source,
        extensions: [
          lineNumbers(), highlightActiveLineGutter(), history(), indentOnInput(), bracketMatching(), cpp(),
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          indentUnit.of("    "),
          EditorView.inputHandler.of(autoExpandBrace),
          EditorState.tabSize.of(4),
          lspCompartment.of([]),
          editableCompartment.of(editableExtensions(false)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              source = update.state.doc.toString();
              dirty = true;
            }
          }),
          EditorView.lineWrapping,
          EditorView.theme({
            "&": { height: "100%", fontSize: "13px" },
            ".cm-scroller": { overflow: "auto", fontFamily: "'Cascadia Code', Consolas, monospace" },
            ".cm-content": { padding: "18px 0", caretColor: "#c25c32" },
            ".cm-gutters": { backgroundColor: "#f4f6f2", border: "none", color: "#8b958a" },
            ".cm-activeLineGutter": { backgroundColor: "#e9eee8", color: "#293d31" },
            ".cm-activeLine": { backgroundColor: "#f7f8f5" },
            "&.cm-focused": { outline: "none" },
            ".cm-cursor": { borderLeftColor: "#c25c32" },
            ".cm-selectionBackground, ::selection": { backgroundColor: "#d8e3d5 !important" }
          })
        ]
      }),
      parent: editorElement
    });

    localforage.getItem<TestCase[]>(storageKey)
      .then((parsed) => {
        if (parsed && Array.isArray(parsed) && parsed.length > 0) {
          testCases = parsed;
          activeTestId = parsed[0].id;
        }
      })
      .catch((error) => {
        console.error("讀取測資失敗:", error);
      });

    return () => {
      disposed = true;
      unlistenResize?.();
      window.removeEventListener("resize", updateViewportMode);
      closeLanguageServer();
      editorView?.destroy();
    };
  });

  // 輸入 "{" 時自動展開成 "{\n    |\n}"；字串、註解內，或游標後方還有內容時維持一般輸入。
  function autoExpandBrace(view: EditorView, from: number, to: number, text: string) {
    if (text !== "{" || from !== to) return false;
    const state = view.state;
    if (/String|Comment|Char/i.test(syntaxTree(state).resolveInner(from, -1).name)) return false;
    const line = state.doc.lineAt(from);
    if (state.doc.sliceString(from, line.to).trim() !== "") return false;
    const base = /^\s*/.exec(line.text)?.[0] ?? "";
    const inner = base + state.facet(indentUnit);
    view.dispatch({
      changes: { from, to, insert: `{\n${inner}\n${base}}` },
      selection: { anchor: from + 2 + inner.length },
      userEvent: "input.type"
    });
    return true;
  }
  function minimizeWindow() { void appWindow?.minimize(); }
  function toggleMaximizeWindow() { void appWindow?.toggleMaximize(); }
  function closeWindow() {
    if (!confirmDiscardChanges()) return;
    void appWindow?.close();
  }
  function editableExtensions(editable: boolean) {
    return [EditorView.editable.of(editable), EditorState.readOnly.of(!editable)];
  }
  function setEditorContent(content: string) {
    source = content;
    editorView?.dispatch({
      changes: { from: 0, to: editorView.state.doc.length, insert: content },
      effects: editableCompartment.reconfigure(editableExtensions(activePath !== ""))
    });
    dirty = false;
  }
  function clearEditor() {
    activePath = "";
    setEditorContent("");
    closeLanguageServer();
  }
  function setNotice(message: string, tone = "neutral") { notice = message; noticeTone = tone; }
  async function connectLanguageServer(path: string) {
    const extension = /\.(cpp|cc|cxx|h|hpp)$/i.test(path);
    if (!extension) {
      closeLanguageServer();
      return;
    }

    const workspacePath = projectPath || path.replace(/[\\/][^\\/]+$/, "");
    try {
      await toolchainInitialization;
    } catch {
      return;
    }
    if (!lspSession || lspSessionWorkspace !== workspacePath) {
      closeLanguageServer();
      clangdStatus = "啟動 clangd...";
      try {
        const session = await invoke<LspSessionInfo>("start_clangd", { workspacePath, documentPath: path });
        const transport = new TauriLspTransport(session.sessionId);
        const unlisten = await listen<string>(session.eventName, (event) => transport.receive(event.payload));
        transport.setUnlisten(unlisten);
        const workspaceFolders = [{ uri: session.rootUri, name: workspacePath.split(/[\\/]/).at(-1) ?? "workspace" }];
        const client = new LanguageServerClient({
          transport,
          autoClose: false,
          rootUri: session.rootUri,
          workspaceFolders,
          documentUri: session.documentUri,
          languageId: "cpp",
          onCapabilities: () => { clangdStatus = "clangd IntelliSense"; },
          onError: (error) => { clangdStatus = `clangd: ${error.message}`; },
          onClose: () => { if (lspSession?.sessionId === session.sessionId) clangdStatus = "clangd 已中斷"; }
        });
        lspSession = session;
        lspSessionWorkspace = workspacePath;
        lspTransport = transport;
        languageClient = client;
      } catch (error) {
        clangdStatus = `IntelliSense 無法啟動：${String(error)}`;
        return;
      }
    }

    if (!lspSession || !languageClient || !lspTransport) return;
    const documentUri = await invoke<string>("file_uri", { path });
    const workspaceFolders = [{ uri: lspSession.rootUri, name: workspacePath.split(/[\\/]/).at(-1) ?? "workspace" }];
    editorView?.dispatch({
      effects: lspCompartment.reconfigure(languageServerWithTransport({
        client: languageClient,
        transport: lspTransport,
        rootUri: lspSession.rootUri,
        workspaceFolders,
        documentUri,
        languageId: "cpp",
        onError: (error) => { clangdStatus = `clangd: ${error.message}`; }
      }))
    });
  }
  let lspSessionWorkspace = "";
  function closeLanguageServer() {
    editorView?.dispatch({ effects: lspCompartment.reconfigure([]) });
    languageClient?.close();
    if (!languageClient) lspTransport?.close();
    languageClient = undefined;
    lspTransport = undefined;
    lspSession = undefined;
    lspSessionWorkspace = "";
    clangdStatus = "等待開啟 C++ 檔案";
  }
  function clamp(value: number, min: number, max: number) { return Math.round(Math.max(min, Math.min(max, value))); }
  function saveLayout() {
    try {
      localStorage.setItem(layoutStorageKey, JSON.stringify({ sidebarWidth, testWidth, consoleHeight, editorHeight, testHeight, sidebarHeight, diffCols: [...diffCols] }));
    } catch { /* Keep resizing available when local storage is unavailable. */ }
  }
  function startResize(event: PointerEvent, kind: ResizeKind) {
    if (event.button !== 0) return;
    activeResize = { kind, pointerId: event.pointerId };
    const target = event.currentTarget;
    if (target instanceof HTMLElement) target.setPointerCapture(event.pointerId);
    event.preventDefault();
  }
  function moveResize(event: PointerEvent) {
    if (!activeResize || activeResize.pointerId !== event.pointerId) return;
    const bounds = workbenchElement.getBoundingClientRect();
    if (activeResize.kind === "sidebar") {
      if (window.innerWidth <= 740) sidebarHeight = clamp(event.clientY - bounds.top, 100, 240);
      else sidebarWidth = clamp(event.clientX - bounds.left, 155, 360);
    } else if (activeResize.kind === "main") {
      if (window.innerWidth <= 980) {
        const editorBounds = editorPanelElement.getBoundingClientRect();
        editorHeight = clamp(event.clientY - editorBounds.top, 240, 560);
      } else testWidth = clamp(bounds.right - event.clientX, 235, 480);
    } else {
      consoleHeight = clamp(bounds.bottom - event.clientY, 150, 420);
    }
  }
  function endResize(event: PointerEvent) {
    if (!activeResize || activeResize.pointerId !== event.pointerId) return;
    activeResize = null;
    saveLayout();
  }
  function diffColumnsStyle() {
    return `grid-template-columns: minmax(0, ${diffCols[0] * 100}fr) 7px minmax(0, ${diffCols[1] * 100}fr) 7px minmax(0, ${diffCols[2] * 100}fr)`;
  }
  // 只移動相鄰兩欄的分界，兩欄寬度總和不變，第三欄不受影響
  function setDiffSplit(index: number, leftFraction: number) {
    const pair = diffCols[index] + diffCols[index + 1];
    const left = Math.max(minDiffCol, Math.min(pair - minDiffCol, leftFraction));
    const next = [...diffCols];
    next[index] = left;
    next[index + 1] = pair - left;
    diffCols = next;
  }
  function startDiffResize(event: PointerEvent, index: number) {
    if (event.button !== 0) return;
    diffResize = { index, pointerId: event.pointerId };
    const target = event.currentTarget;
    if (target instanceof HTMLElement) target.setPointerCapture(event.pointerId);
    event.preventDefault();
  }
  function moveDiffResize(event: PointerEvent) {
    if (!diffResize || diffResize.pointerId !== event.pointerId || !diffColumnsElement) return;
    const available = diffColumnsElement.clientWidth - 14;
    if (available <= 0) return;
    const leftColumn = diffColumnsElement.children[diffResize.index * 2];
    if (!(leftColumn instanceof HTMLElement)) return;
    setDiffSplit(diffResize.index, (event.clientX - leftColumn.getBoundingClientRect().left - 3.5) / available);
  }
  function endDiffResize(event: PointerEvent) {
    if (!diffResize || diffResize.pointerId !== event.pointerId) return;
    diffResize = null;
    saveLayout();
  }
  function resizeDiffWithKeyboard(event: KeyboardEvent, index: number) {
    const step = event.shiftKey ? 0.06 : 0.02;
    if (event.key === "ArrowLeft") setDiffSplit(index, diffCols[index] - step);
    else if (event.key === "ArrowRight") setDiffSplit(index, diffCols[index] + step);
    else return;
    event.preventDefault();
    saveLayout();
  }
  function resetDiffColumns() {
    diffCols = [1 / 3, 1 / 3, 1 / 3];
    saveLayout();
  }
  function resizeWithKeyboard(event: KeyboardEvent, kind: ResizeKind) {
    const step = event.shiftKey ? 32 : 12;
    if (kind === "sidebar") {
      if (window.innerWidth <= 740) {
        if (event.key === "ArrowUp") sidebarHeight = clamp(sidebarHeight - step, 100, 240);
        else if (event.key === "ArrowDown") sidebarHeight = clamp(sidebarHeight + step, 100, 240);
        else return;
      } else {
        if (event.key === "ArrowLeft") sidebarWidth = clamp(sidebarWidth - step, 155, 360);
        else if (event.key === "ArrowRight") sidebarWidth = clamp(sidebarWidth + step, 155, 360);
        else return;
      }
    } else if (kind === "main") {
      if (window.innerWidth <= 980) {
        if (event.key === "ArrowUp") editorHeight = clamp(editorHeight - step, 240, 560);
        else if (event.key === "ArrowDown") editorHeight = clamp(editorHeight + step, 240, 560);
        else return;
      } else {
        if (event.key === "ArrowLeft") testWidth = clamp(testWidth + step, 235, 480);
        else if (event.key === "ArrowRight") testWidth = clamp(testWidth - step, 235, 480);
        else return;
      }
    } else {
      if (event.key === "ArrowUp") consoleHeight = clamp(consoleHeight + step, 150, 420);
      else if (event.key === "ArrowDown") consoleHeight = clamp(consoleHeight - step, 150, 420);
      else return;
    }
    event.preventDefault();
    saveLayout();
  }
  function confirmDiscardChanges() {
    if (!dirty) return true;
    return window.confirm("目前檔案有尚未儲存的變更，要捨棄嗎？");
  }
  function joinPath(folder: string, filename: string) { return `${folder}${folder.includes("\\") ? "\\" : "/"}${filename}`; }
  function relativeFile(path: string) {
    return projectPath ? path.slice(projectPath.length).replace(/^[\\/]/, "") : path.split(/[\\/]/).at(-1) ?? path;
  }
  async function refreshFiles(path = projectPath) {
    if (path) filePaths = await invoke<string[]>("list_source_files", { projectPath: path });
  }
  async function loadFile(path: string, discardConfirmed = false) {
    if (!discardConfirmed && !confirmDiscardChanges()) return;
    try {
      const contents = await invoke<string>("read_source", { path });
      activePath = path;
      setEditorContent(contents);
      setNotice(`已開啟 ${relativeFile(path)}`);
      await connectLanguageServer(path);
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function openProject() {
    if (!confirmDiscardChanges()) return;
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== "string") return;
      projectPath = selected;
      await refreshFiles(selected);
      if (filePaths.length) await loadFile(filePaths[0], true);
      else {
        clearEditor();
        setNotice("專案已開啟，新增一個 C++ 檔案開始撰寫");
      }
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function openSourceFile() {
    try {
      const selected = await open({ multiple: false, defaultPath: projectPath || undefined, filters: [{ name: "C++ 原始碼", extensions: ["cpp", "cc", "cxx", "h", "hpp"] }] });
      if (typeof selected !== "string") return;
      projectPath = selected.replace(/[\\/][^\\/]+$/, "");
      await loadFile(selected);
      await refreshFiles();
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function createProject(event: SubmitEvent) {
    event.preventDefault();
    if (!projectName.trim()) return;
    try {
      const parent = await open({ directory: true, multiple: false });
      if (typeof parent !== "string") return;
      projectPath = await invoke<string>("create_project", { parentPath: parent, name: projectName.trim() });
      projectName = "";
      showNewProject = false;
      await refreshFiles();
      await loadFile(joinPath(projectPath, "main.cpp"));
      setNotice("專案已建立", "success");
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function newSourceFile() {
    if (!confirmDiscardChanges()) return;
    try {
      const selected = await save({
        title: "新增 C++ 檔案",
        defaultPath: projectPath ? joinPath(projectPath, "main.cpp") : "main.cpp",
        filters: [{ name: "C++ 原始碼", extensions: ["cpp"] }]
      });
      if (!selected) { setNotice("已取消新增檔案"); return; }
      const path = /\.(cpp|cc|cxx|h|hpp)$/i.test(selected) ? selected : `${selected}.cpp`;
      await invoke("save_source", { path, content: starterCode });
      if (!projectPath || !path.startsWith(projectPath)) projectPath = path.replace(/[\\/][^\\/]+$/, "");
      await refreshFiles();
      await loadFile(path, true);
      setNotice(`已建立 ${relativeFile(path)}`, "success");
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function saveCurrent(): Promise<boolean> {
    const path = activePath;
    if (!path) { setNotice("請先新增或開啟檔案", "error"); return false; }
    try {
      await invoke("save_source", { path, content: source });
      dirty = false;
      await refreshFiles();
      await connectLanguageServer(path);
      setNotice("檔案已儲存", "success");
      return true;
    } catch (error) { setNotice(String(error), "error"); return false; }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const isSaveShortcut =
      (event.ctrlKey || event.metaKey) &&
      !event.shiftKey &&
      !event.altKey &&
      event.key.toLowerCase() === "s";

    if (!isSaveShortcut) return;

    event.preventDefault();

    if (!hasFile || busy !== "" || event.repeat) return;

    void saveCurrent();
  }
  async function compileCurrent(): Promise<boolean> {
    if (!toolchainsReady) { setNotice(toolchainStatus, "error"); return false; }
    if (!(await saveCurrent())) return false;
    busy = "compile";
    compilerOutput = "正在呼叫 G++...";
    consoleTab = "build";
    try {
      const result = await invoke<CompileResult>("compile_source", { path: activePath });
      compilerOutput = result.output || "Build Successful";
      setNotice(result.success ? "Build Successful" : "Build Failed", result.success ? "success" : "error");
      return result.success;
    } catch (error) { compilerOutput = String(error); setNotice(String(error), "error"); return false; }
    finally { busy = ""; }
  }
  async function runProgram() {
    if (!(await compileCurrent())) return;
    busy = "run";
    consoleTab = "output";
    const runId = crypto.randomUUID();
    activeRunId = runId;
    cancelRequested = false;
    try {
      await invoke("register_run", { runId });
      const result = await invoke<RunResult>("run_source", { runId, path: activePath, input: activeTest?.input ?? "", timeoutMs });
      stdout = result.stdout;
      stderr = result.stderr;
      if (result.cancelled) setNotice("程式已停止");
      else setNotice(
          result.timedOut ? "Time Limit Exceeded" : result.exitCode === 0 ? `執行完成 · ${result.executionTimeMs} ms` : `Runtime Error · Exit ${result.exitCode}`,
          result.timedOut || result.exitCode !== 0 ? "error" : "success"
        );
    } catch (error) { stderr = String(error); setNotice(String(error), "error"); }
    finally {
      try { await invoke("finish_run", { runId }); } catch { /* Preserve the completed run state. */ }
      activeRunId = "";
      busy = "";
    }
  }
  async function stopCurrentRun() {
    if (!activeRunId) return;
    cancelRequested = true;
    setNotice("正在停止程式...");
    try { await invoke("cancel_run", { runId: activeRunId }); }
    catch (error) { setNotice(String(error), "error"); }
  }
  function persistTests() { void localforage.setItem(storageKey, $state.snapshot(testCases)); }
  function addTestCase() {
    const id = crypto.randomUUID();
    testCases = [...testCases, { id, name: "", input: "", expectedOutput: "" }];
    activeTestId = id;
    persistTests();
    showTestManager = true;
  }
  function removeTestCase(id: string) {
    if (testCases.length === 1) return;
    testCases = testCases.filter((testCase) => testCase.id !== id);
    if (activeTestId === id) activeTestId = testCases[0].id;
    persistTests();
  }
  function openTestManager() {
    showTestManager = true;
  }
  function closeTestManager() {
    showTestManager = false;
  }
  function updateTest(id: string, field: "name" | "input" | "expectedOutput", value: string) {
    testCases = testCases.map((testCase) => testCase.id === id ? { ...testCase, [field]: value } : testCase);
    persistTests();
  }
  async function runTests(all: boolean) {
    if (!toolchainsReady) { setNotice(toolchainStatus, "error"); return; }
    if (!(await saveCurrent())) return;
    busy = "test";
    compilerOutput = "正在編譯測試程式...";
    consoleTab = "build";
    let runId = "";
    try {
      const compiled = await invoke<CompileResult>("compile_source", { path: activePath });
      compilerOutput = compiled.output || "Build Successful";
      if (!compiled.success) { setNotice("Build Failed，測資未執行", "error"); return; }
      const casesToRun = all ? [...testCases] : testCases.filter((testCase) => testCase.id === activeTestId);
      runId = crypto.randomUUID();
      activeRunId = runId;
      cancelRequested = false;
      await invoke("register_run", { runId });
      for (const testCase of casesToRun) {
        if (cancelRequested) break;
        const result = await invoke<RunResult>("run_source", { runId, path: activePath, input: testCase.input, timeoutMs });
        if (result.cancelled) { cancelRequested = true; break; }
        let status: ResultStatus = "AC";
        let firstDifference: number | null = null;
        if (result.timedOut) status = "TLE";
        else if (result.exitCode !== 0) status = "RE";
        else {
          const comparison = await invoke<CompareResult>("compare_output", { expected: testCase.expectedOutput, actual: result.stdout });
          status = comparison.accepted ? "AC" : "WA";
          firstDifference = comparison.firstDifference;
        }
        testCases = testCases.map((item) => item.id === testCase.id
          ? { ...item, status, actualOutput: result.stdout, stderr: result.stderr, executionTimeMs: result.executionTimeMs, firstDifference }
          : item);
        stdout = result.stdout;
        stderr = result.stderr;
      }
      persistTests();
      if (cancelRequested) {
        consoleTab = "output";
        setNotice("測試已停止");
        return;
      }
      const accepted = casesToRun.every((testCase) => testCases.find((item) => item.id === testCase.id)?.status === "AC");
      consoleTab = casesToRun.some((testCase) => testCases.find((item) => item.id === testCase.id)?.status === "WA") ? "diff" : "output";
      setNotice(accepted ? `${casesToRun.length} 組測資全部通過` : "測資執行完成，請檢查結果", accepted ? "success" : "error");
    } catch (error) { stderr = String(error); setNotice(String(error), "error"); }
    finally {
      if (runId) {
        try { await invoke("finish_run", { runId }); } catch { /* Preserve the completed test state. */ }
      }
      activeRunId = "";
      busy = "";
    }
  }
</script>

<svelte:head>
  <title>競程工作台 · IDE for Exam</title>
  <meta name="description" content="Windows 本機 C++ 編輯、編譯與測資判題工作台" />
</svelte:head>
<svelte:window onkeydown={handleGlobalKeydown} />

<div class="app-shell">
  <div class="titlebar" data-tauri-drag-region>
    <div class="window-controls">
      <button type="button" class="window-button" title="最小化" aria-label="最小化" onclick={minimizeWindow}><Minus size={16} /></button>
      <button type="button" class="window-button" title={isMaximized ? "取消最大化" : "最大化"} aria-label={isMaximized ? "還原" : "最大化"} onclick={toggleMaximizeWindow}>{#if isMaximized}<Copy size={13} />{:else}<Square size={13} />{/if}</button>
      <button type="button" class="window-button window-close" title="關閉" aria-label="關閉" onclick={closeWindow}><X size={17} /></button>
    </div>
  </div>
  <div class="actionbar" data-tauri-drag-region>
    <div class="file-actions">
      <button class="text-action" onclick={() => showNewProject = true}><FolderPlus size={15} />建立專案</button>
      <button class="text-action" onclick={openProject}><FolderOpen size={15} />開啟專案</button>
      <span class="action-divider"></span>
      <button class="text-action" onclick={newSourceFile}><CirclePlus size={15} />新檔案</button>
      <button class="text-action" onclick={openSourceFile}><FolderOpen size={15} />開啟檔案</button>
      <button class="text-action" onclick={saveCurrent} disabled={!hasFile}><Save size={15} />儲存檔案</button>
    </div>
    <div class="run-actions">
      <label class="timeout-field" title="程式逾時上限"><Clock3 size={14} /><input type="number" min="100" max="300000" step="100" bind:value={timeoutMs} aria-label="執行逾時毫秒" /><span>ms</span></label>
      <button class="compile-button" onclick={compileCurrent} disabled={busy !== "" || !toolchainsReady || !hasFile}><Settings2 size={15} />編譯</button>
      <button class="run-button" onclick={runProgram} disabled={busy !== "" || !toolchainsReady || !hasFile}><Play size={15} fill="currentColor" />執行</button>
      <button class="test-button" onclick={() => runTests(true)} disabled={busy !== "" || !toolchainsReady || !hasFile}><CircleCheck size={15} />全部測試</button>
      {#if activeRunId && (busy === "run" || busy === "test")}<button class="stop-button" onclick={stopCurrentRun}><Square size={13} fill="currentColor" />停止</button>{/if}
    </div>
  </div>
  <main
    class="workbench"
    class:resizing={activeResize !== null}
    bind:this={workbenchElement}
    style={`--sidebar-width:${sidebarWidth}px;--sidebar-height:${sidebarHeight}px;--test-width:${testWidth}px;--console-height:${consoleHeight}px;--editor-height:${editorHeight}px;--test-height:${testHeight}px`}
  >
    <aside class="sidebar">
      <section class="side-section files-section">
        <div class="section-heading"><span>專案檔案 <small>{filePaths.length}</small></span><button class="mini-icon" title="新增 C++ 檔案" aria-label="新增 C++ 檔案" onclick={newSourceFile}><CirclePlus size={15} /></button></div>
        {#if filePaths.length}<ul class="file-list">{#each filePaths as file (file)}<li><button class:active={file === activePath} class="file-item" onclick={() => loadFile(file)}><FileCode2 size={15} /><span>{relativeFile(file)}</span>{#if file === activePath && dirty}<i class="file-dirty"></i>{/if}</button></li>{/each}</ul>
        {:else}<p class="empty-note">開啟資料夾以瀏覽來源檔</p>{/if}
      </section>
    </aside>
    <section class="editor-panel" bind:this={editorPanelElement}>
      <div class="editor-tabbar"><div class="active-file-tab"><FileCode2 size={15} /><span>{activePath ? relativeFile(activePath) : "尚未開啟檔案"}</span>{#if dirty}<i></i>{/if}</div><div class="editor-shortcut"><Search size={13} /><span>Ctrl F 搜尋</span></div></div>
      <div class="editor-wrap">
        <div class="editor-host" bind:this={editorElement}></div>
        {#if !hasFile}
          <div class="editor-empty">
            <FileCode2 size={30} />
            <strong>尚未開啟任何檔案</strong>
            <p>新增檔案時需先選擇儲存位置，儲存後即可開始編輯。</p>
            <div class="editor-empty-actions">
              <button type="button" class="empty-primary" onclick={newSourceFile}><CirclePlus size={15} />新增檔案</button>
              <button type="button" class="empty-secondary" onclick={openSourceFile}><FolderOpen size={15} />開啟檔案</button>
            </div>
          </div>
        {/if}
      </div>
      <div class="editor-status"><span>{activePath ? relativeFile(activePath) : "無檔案"}</span><span class:ready={clangdStatus === "clangd IntelliSense"} class:unavailable={clangdStatus.startsWith("IntelliSense 無法") || clangdStatus.startsWith("clangd:")} class="clangd-status" title={clangdStatus}>{clangdStatus}</span><span>C++17</span><span>UTF-8</span><span>LF</span></div>
    </section>
    <aside class="case-sidebar">
      <div class="panel-title-row"><div><span class="eyebrow">LOCAL JUDGE</span><h2>測資列表 <small>len: {testCases.length}</small></h2></div><button class="mini-icon" title="測資編輯器" aria-label="測資編輯器" onclick={openTestManager}><Settings2 size={15} /></button></div>
      {#if testCases.length}
        <ul class="case-list">{#each testCases as testCase, index (testCase.id)}<li class="case-row">
          <button
            class="case-chip"
            class:case-ac={testCase.status === "AC"}
            class:case-wa={testCase.status === "WA"}
            class:case-other={testCase.status !== "AC" && testCase.status !== "WA"}
            class:active={activeTestId === testCase.id}
            onclick={() => activeTestId = testCase.id}
            title={`檢視 ${testCase.name}`}
          >
            <span class="case-index">#{String(index + 1).padStart(2, "0")} {#if testCase.name != ""}<strong>{testCase.name}</strong>{/if}</span>
            <span class="case-result">{testCase.status ?? "—"}</span>
          </button>
          <button type="button" class="mini-icon remove-case" title="刪除測資" aria-label="刪除測資" onclick={() => removeTestCase(testCase.id)} disabled={testCases.length === 1}><Trash2 size={13} /></button>
        </li>{/each}</ul>
      {:else}<p class="empty-note">尚無測資，點擊右上角開啟主控台新增。</p>{/if}
    </aside>
    <section class="console-panel">
      <div class="console-header"><div class="console-tabs" role="tablist" aria-label="執行結果">
        <button class:selected={consoleTab === "output"} role="tab" aria-selected={consoleTab === "output"} onclick={() => consoleTab = "output"}><Terminal size={14} />程式輸出</button>
        <button class:selected={consoleTab === "build"} role="tab" aria-selected={consoleTab === "build"} onclick={() => consoleTab = "build"}><Settings2 size={14} />編譯器</button>
        <button class:selected={consoleTab === "diff"} role="tab" aria-selected={consoleTab === "diff"} onclick={() => consoleTab = "diff"}><Code2 size={14} />輸出差異</button>
      </div><div class="console-state" class:error={noticeTone === "error"} class:success={noticeTone === "success"}>{#if busy}<span class="working-indicator"></span>{/if}{notice}</div></div>
      {#if consoleTab === "output"}<div class="output-columns"><div class="output-block"><div class="output-label">STDOUT</div><pre>{stdout || "執行結果將顯示於此"}</pre></div><div class="output-block stderr-block"><div class="output-label">STDERR</div><pre>{stderr || "無錯誤輸出"}</pre></div></div>
      {:else if consoleTab === "build"}<pre class="compiler-output">{compilerOutput || "編譯器訊息將顯示於此"}</pre>
      {:else if activeTest}
        {@const diff = activeTest.actualOutput === undefined ? null : buildDiff(activeTest.expectedOutput, activeTest.actualOutput)}
        {@const inputLines = numberedLines(activeTest.input)}
        {@const expectedLines = numberedLines(activeTest.expectedOutput)}
        {@const actualLines = numberedLines(activeTest.actualOutput ?? "")}
        {#snippet colSplitter(index: number)}<button type="button" class="splitter col-splitter" class:active={diffResize?.index === index} role="slider" aria-orientation="vertical" aria-valuemin={12} aria-valuemax={76} aria-valuenow={Math.round(diffCols[index] * 100)} aria-label={index === 0 ? "調整 STANDARD INPUT 與 EXPECTED 的寬度" : "調整 EXPECTED 與 ACTUAL 的寬度"} title="拖曳調整欄寬（雙擊還原）" onpointerdown={(event) => startDiffResize(event, index)} onpointermove={moveDiffResize} onpointerup={endDiffResize} onpointercancel={endDiffResize} onkeydown={(event) => resizeDiffWithKeyboard(event, index)} ondblclick={resetDiffColumns}><span></span></button>{/snippet}
        <div class="diff-columns" bind:this={diffColumnsElement} style={diffColumnsStyle()}>
          <div><div class="output-label">STANDARD INPUT</div>{#if activeTest.input}<pre class="numbered" style={gutterStyle(inputLines.length)}>{#each inputLines as line, i}<span class="ln" data-n={i + 1}>{line}</span>{/each}</pre>{:else}<pre>（無輸入）</pre>{/if}</div>
          {@render colSplitter(0)}
          <div><div class="output-label">EXPECTED</div><pre class="numbered" style={gutterStyle(expectedLines.length)}>{#each expectedLines as line, i}<span class="ln" data-n={i + 1}>{line}</span>{/each}</pre></div>
          {@render colSplitter(1)}
          <div>
            <div class="output-label actual-label">ACTUAL{#if diff && showDiffMarks && diff.hasDiff}<span class="diff-count">{diff.diffCount} 行有差異</span>{/if}{#if diff}<button type="button" class="diff-toggle" class:on={showDiffMarks} aria-pressed={showDiffMarks} title="切換原始輸出／差異標示" onclick={() => showDiffMarks = !showDiffMarks}>{showDiffMarks ? "Show Changes：On" : "Show Changes：Off"}</button>{/if}</div>
            {#if diff && showDiffMarks}<pre class="numbered" style={gutterStyle(diff.lineCount)}>{#each diff.rows as row}<span class="ln" data-n={row.line ?? ""}>{#if row.kind === "missing"}<span class="diff-ghost">{row.segments[0].text || " "}</span>{:else}{#each row.segments as seg}{#if seg.missing}<span class="diff-missing" title={"缺少：" + seg.missing}></span>{:else if seg.bad}<span class="diff-bad">{seg.text}</span>{:else}{seg.text}{/if}{/each}{/if}</span>{/each}</pre>
            {:else if diff}<pre class="numbered" style={gutterStyle(actualLines.length)}>{#each actualLines as line, i}<span class="ln" data-n={i + 1}>{line}</span>{/each}</pre>
            {:else}<pre>尚未執行此測資</pre>{/if}
          </div>
          {#if diff?.hasDiff}<p class="diff-note">{diff.firstNote}</p>
          {:else if diff && activeTest.status === "WA"}<p class="diff-note">僅有結尾換行等不可見差異</p>{/if}
        </div>
      {:else}<pre class="compiler-output">請先建立測資。</pre>{/if}
    </section>
    <button type="button" class="splitter splitter-sidebar" role="slider" aria-orientation={viewportMode === "mobile" ? "horizontal" : "vertical"} aria-valuemin={viewportMode === "mobile" ? 100 : 155} aria-valuemax={viewportMode === "mobile" ? 240 : 360} aria-valuenow={viewportMode === "mobile" ? sidebarHeight : sidebarWidth} aria-label="調整側欄大小" title="拖曳調整側欄大小" onpointerdown={(event) => startResize(event, "sidebar")} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} onkeydown={(event) => resizeWithKeyboard(event, "sidebar")}><span></span></button>
    <button type="button" class="splitter splitter-main" role="slider" aria-orientation={viewportMode === "desktop" ? "vertical" : "horizontal"} aria-valuemin={viewportMode === "desktop" ? 235 : 240} aria-valuemax={viewportMode === "desktop" ? 480 : 560} aria-valuenow={viewportMode === "desktop" ? testWidth : editorHeight} aria-label="調整編輯器與測資比例" title="拖曳調整編輯器與測資比例" onpointerdown={(event) => startResize(event, "main")} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} onkeydown={(event) => resizeWithKeyboard(event, "main")}><span></span></button>
    <button type="button" class="splitter splitter-console" role="slider" aria-orientation="horizontal" aria-valuemin="150" aria-valuemax="420" aria-valuenow={consoleHeight} aria-label="調整輸出面板高度" title="拖曳調整輸出面板高度" onpointerdown={(event) => startResize(event, "console")} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} onkeydown={(event) => resizeWithKeyboard(event, "console")}><span></span></button>
  </main>
  <footer class="statusbar"><span class="status-project"><span class="compiler-dot"></span>{projectPath || "本機工作區"}</span><span class="status-toolchain" class:ready={toolchainsReady}>{toolchainStatus}</span><span>競程工作台 <b>0.1.0</b></span></footer>
</div>

{#if showNewProject}<div class="modal-backdrop"><dialog open class="project-modal" aria-labelledby="new-project-title">
  <div class="modal-icon"><FolderPlus size={19} /></div><h2 id="new-project-title">建立 C++ 專案</h2><p>選擇儲存位置後，工作台會建立 main.cpp。</p>
  <form onsubmit={createProject}><label for="project-name">專案名稱</label><input id="project-name" bind:value={projectName} placeholder="例如：apcs-practice" /><div class="modal-actions"><button type="button" class="cancel-button" onclick={() => showNewProject = false}>取消</button><button type="submit" class="confirm-button" disabled={!projectName.trim()}>選擇位置並建立</button></div></form>
</dialog></div>{/if}

{#if showTestManager}<div class="modal-backdrop" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) closeTestManager(); }}><dialog open class="project-modal test-manager-modal" aria-labelledby="test-manager-title">
  <div class="modal-title-row">
    <div class="modal-icon"><Code2 size={19} /></div>
    <button type="button" class="mini-icon modal-close" title="關閉" aria-label="關閉" onclick={closeTestManager}><CircleX size={18} /></button>
  </div>
  <h2 id="test-manager-title">測資編輯器</h2>
  <div class="test-manager-body">
    <div class="test-manager-editor">
      {#if activeTest}
        <div class="case-name-row">
          <input class="case-name" value={activeTest.name} aria-label="測資名稱" placeholder="請輸入測資名稱" onchange={(event) => updateTest(activeTest.id, "name", event.currentTarget.value)} />
          <button class="run-case-button" onclick={() => runTests(false)} disabled={busy !== "" || !hasFile} title="執行目前測資"><Play size={14} fill="currentColor" /></button>
          <button type="button" class="mini-icon remove-case" title="刪除測資" aria-label="刪除測資" onclick={() => removeTestCase(activeTest.id)} disabled={testCases.length === 1}><Trash2 size={14} /></button>
        </div>
        <label class="code-field-label" for="test-input">STANDARD INPUT</label><textarea id="test-input" class="case-textarea" value={activeTest.input} oninput={(event) => updateTest(activeTest.id, "input", event.currentTarget.value)} spellcheck="false"></textarea>
        <label class="code-field-label" for="test-expected">EXPECTED OUTPUT</label><textarea id="test-expected" class="case-textarea expected-area" value={activeTest.expectedOutput} oninput={(event) => updateTest(activeTest.id, "expectedOutput", event.currentTarget.value)} spellcheck="false"></textarea>
        <div class="case-result-row">{#if activeTest.status}<span class:status-ac={activeTest.status === "AC"} class:status-fail={activeTest.status !== "AC"} class="result-pill">{activeTest.status}</span>{:else}<span class="result-placeholder">尚未執行</span>{/if}{#if activeTest.executionTimeMs !== undefined}<span>{activeTest.executionTimeMs} ms</span>{/if}</div>
      {:else}
        <p class="empty-note">尚無測資，請於右側新增一筆。</p>
      {/if}
    </div>
    <aside class="test-manager-list">
      <div class="panel-title-row"><h2>測資列表 <small>len: {testCases.length}</small></h2><button class="mini-icon" title="新增測資" aria-label="新增測資" onclick={addTestCase}><CirclePlus size={15} /></button></div>
      {#if testCases.length}
        <ul class="case-list">{#each testCases as testCase, index(testCase.id)}<li class="case-row">
          <button
            class="case-chip"
            class:case-ac={testCase.status === "AC"}
            class:case-wa={testCase.status === "WA"}
            class:case-other={testCase.status !== "AC" && testCase.status !== "WA"}
            class:active={activeTestId === testCase.id}
            onclick={() => activeTestId = testCase.id}
            title={`編輯 ${testCase.name}`}
          >
            <span class="case-index">#{String(index + 1).padStart(2, "0")} {#if testCase.name != ""}<strong>{testCase.name}</strong>{/if}</span>
            <span class="case-result">{testCase.status ?? "—"}</span>
          </button>
          <button type="button" class="mini-icon remove-case" title="刪除測資" aria-label="刪除測資" onclick={() => removeTestCase(testCase.id)} disabled={testCases.length === 1}><Trash2 size={13} /></button>
        </li>{/each}</ul>
      {:else}<p class="empty-note">尚無測資，點擊上方新增。</p>{/if}
    </aside>
  </div>
</dialog></div>{/if}

<style>
  :global(*) { box-sizing: border-box; }
  :global(html), :global(body) { margin: 0; min-width: 720px; min-height: 100%; background: #e8ece7; }
  :global(body) { color: #26352d; font-family: "Segoe UI Variable", "Segoe UI", sans-serif; font-size: 13px; }
  :global(button), :global(input), :global(textarea) { font: inherit; }
  :global(button) { color: inherit; }
  .app-shell { display: flex; flex-direction: column; width: 100%; height: 100vh; min-height: 620px; --pad-x: 14px; --pad-y: 8px; padding: var(--pad-y) var(--pad-x) 8px; gap: 8px; background: radial-gradient(ellipse at 4% 0%, #f5f7f2 0%, #e8ece7 49%, #e4e9e3 100%); }
  .actionbar, .workbench, .statusbar { border: 1px solid #d6ddd5; background: #fbfcf9; }
  .eyebrow { display: block; color: #879287; font-size: 9px; font-weight: 750; }
  .dirty-dot, .file-dirty { width: 7px; height: 7px; flex: 0 0 7px; border-radius: 50%; background: #c56b40; }
  .icon-button, .mini-icon { display: grid; place-items: center; border: 0; background: transparent; color: #68776d; cursor: pointer; }
  .icon-button { width: 32px; height: 32px; border-radius: 5px; }
  .icon-button:hover, .mini-icon:hover { color: #315841; background: #eef2ec; }
  .save-button, .text-action, .compile-button, .run-button, .test-button, .stop-button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; border: 1px solid transparent; border-radius: 4px; cursor: pointer; font-size: 11px; font-weight: 650; }
  .save-button { height: 32px; padding: 0 12px; border-color: #dce2da; color: #43564a; background: #fff; }
  .save-button:hover, .text-action:hover { background: #f0f3ee; }
  .actionbar { display: flex; align-items: center; justify-content: space-between; min-height: 44px; flex: 0 0 44px; padding: 0 12px; border-radius: 3px; box-shadow: 0 2px 8px #2033270b; }
  .titlebar { display: flex; height: 30px; flex: 0 0 30px; justify-content: flex-end; margin: calc(var(--pad-y) * -1) calc(var(--pad-x) * -1) -4px; }
  .window-controls { display: flex; height: 100%; }
  .window-button { display: grid; width: 46px; height: 100%; place-items: center; padding: 0; border: 0; border-radius: 0; color: #5d6c61; background: transparent; cursor: pointer; }
  .window-button:hover { color: #315841; background: #eef2ec; }
  .window-close:hover { color: #fff; background: #d1503a; }
  .action-divider { width: 1px; height: 20px; margin: 0 3px; background: #e0e5df; }
  .file-actions, .run-actions { display: flex; align-items: center; gap: 6px; }
  .text-action { height: 30px; padding: 0 9px; color: #59675d; background: transparent; }
  .run-actions { gap: 8px; }
  .timeout-field { display: flex; align-items: center; gap: 5px; height: 29px; padding: 0 7px; border: 1px solid #e0e5df; border-radius: 4px; color: #758178; background: #fff; }
  .timeout-field input { width: 46px; padding: 0; border: 0; outline: 0; color: #415247; background: transparent; font-variant-numeric: tabular-nums; }
  .timeout-field span { color: #8d978e; font-size: 10px; }
  .compile-button, .run-button, .test-button { height: 30px; padding: 0 11px; }
  .compile-button { border-color: #d9e0d7; color: #425448; background: #f8faf6; }
  .run-button { color: white; background: #315841; }
  .run-button:hover { background: #254a34; }
  .test-button { border-color: #e3d6cc; color: #9b5738; background: #fbf5f0; }
  .test-button:hover { background: #f6eae0; }
  .stop-button { height: 30px; padding: 0 9px; border-color: #edcbbf; color: #a64f38; background: #fff7f3; }
  .stop-button:hover { background: #fcebe4; }
  button:disabled { opacity: .48; cursor: not-allowed; }
  .workbench { display: grid; min-height: 0; flex: 1; grid-template-columns: var(--sidebar-width) 6px minmax(300px, 1fr) 6px var(--test-width); grid-template-rows: minmax(240px, 1fr) 6px var(--console-height); overflow: hidden; border-radius: 3px; box-shadow: 0 5px 18px #2033270b; }
  .sidebar { display: flex; min-height: 0; flex-direction: column; grid-column: 1; grid-row: 1 / 4; border-right: 1px solid #e1e6df; background: #f8faf6; }
  .side-section { padding: 12px 9px 8px; }
  .files-section { min-height: 0; flex: 1; overflow: auto; }
  .section-heading { display: flex; align-items: center; justify-content: space-between; padding: 1px 7px 9px; color: #7b877d; font-size: 10px; font-weight: 750; }
  .section-heading small { margin-left: 4px; color: #a0aaa0; font-size: 10px; font-weight: 550; }
  .mini-icon { width: 25px; height: 25px; border-radius: 4px; }
  .file-list { margin: 0; padding: 0; list-style: none; }
  .file-item { display: flex; width: 100%; align-items: center; gap: 8px; min-height: 30px; padding: 0 8px; border: 0; border-radius: 4px; color: #647268; background: transparent; text-align: left; cursor: pointer; }
  .file-item span { overflow: hidden; flex: 1; text-overflow: ellipsis; white-space: nowrap; }
  .file-item:hover { background: #eff3ed; }
  .file-item.active { color: #2e563c; background: #e9f0e7; font-weight: 650; }
  .file-item :global(svg) { flex: 0 0 auto; color: #738d75; }
  .file-dirty { margin-left: auto; }
  .case-list { display: flex; min-height: 0; flex-direction: column; gap: 6px; margin: 0; padding: 0; overflow: auto; list-style: none; }
  .case-row { display: flex; align-items: center; gap: 6px; }
  .case-chip { display: flex; flex: 1; min-width: 0; align-items: center; justify-content: space-between; min-height: 34px; padding: 0 10px; border: 1.5px solid #e4e9e2; border-radius: 5px; color: #45564a; background: #fff; text-align: left; cursor: pointer; }
  .case-chip.case-ac:hover { background: #f1f8f2; }
  .case-chip.case-ac.active { background: #e8f4ea; box-shadow: 0 0 0 1.5px #4c9a5f inset; }
  .case-chip.case-wa:hover { background: #fdf4f2; }
  .case-chip.case-wa.active { background: #fbe9e5; box-shadow: 0 0 0 1.5px #d1503a inset; }
  .case-chip.case-other:hover { background: #fef8ef; }
  .case-chip.case-other.active { background: #fcf0e0; box-shadow: 0 0 0 1.5px #d78a3a inset; }
  .case-row .remove-case { flex: 0 0 auto; }
  .case-index { color: #647268; font-family: "Cascadia Code", Consolas, monospace; font-size: 11px; font-weight: 650; }
  .case-result { padding: 2px 7px; border-radius: 3px; color: #647268; background: #eef2ec; font-size: 9px; font-weight: 800; letter-spacing: .02em; }
  .case-chip.case-ac { border-color: #4c9a5f; }
  .case-chip.case-ac .case-result { color: #29623b; background: #e3f3e5; }
  .case-chip.case-wa { border-color: #d1503a; }
  .case-chip.case-wa .case-result { color: #a23a26; background: #fbe6e1; }
  .case-chip.case-other { border-color: #d78a3a; }
  .case-chip.case-other .case-result { color: #a2621c; background: #fbedd9; }
  .status-ac { color: #3c7954 !important; }
  .status-fail { color: #bc6544 !important; }
  .status-toolchain { color: #8a958b; }
  .status-toolchain.ready { color: #4c7a55; }
  .compiler-dot { width: 7px; height: 7px; border-radius: 50%; background: #82a875; box-shadow: 0 0 0 3px #82a87520; }
  .empty-note { margin: 5px 8px; color: #929d93; font-size: 11px; line-height: 1.6; }
  .editor-panel { display: flex; min-width: 0; min-height: 0; flex-direction: column; grid-column: 3; grid-row: 1; border-right: 1px solid #e1e6df; }
  .editor-tabbar { display: flex; height: 39px; flex: 0 0 39px; align-items: stretch; justify-content: space-between; border-bottom: 1px solid #e5e9e3; background: #f8faf6; }
  .active-file-tab { display: flex; min-width: 0; align-items: center; gap: 8px; padding: 0 13px; border-bottom: 2px solid #52775c; color: #3f5446; font-size: 11px; }
  .active-file-tab span { overflow: hidden; max-width: 220px; text-overflow: ellipsis; white-space: nowrap; }
  .active-file-tab :global(svg) { flex: 0 0 auto; color: #758e76; }
  .active-file-tab i { width: 6px; height: 6px; border-radius: 50%; background: #c56b40; }
  .editor-shortcut { display: flex; align-items: center; gap: 5px; padding: 0 12px; color: #9aa39a; font-size: 10px; }
  .editor-wrap { position: relative; display: flex; min-height: 0; flex: 1; flex-direction: column; }
  .editor-host { min-height: 0; flex: 1; overflow: hidden; background: #fbfcf9; }
  .editor-empty { position: absolute; inset: 0; z-index: 2; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 24px; text-align: center; color: #6b7a70; background: #fbfcf9; }
  .editor-empty :global(svg) { color: #9aae9c; }
  .editor-empty strong { color: #34443a; font-size: 14px; font-weight: 650; }
  .editor-empty p { margin: 0; font-size: 12px; }
  .editor-empty-actions { display: flex; gap: 8px; margin-top: 10px; }
  .editor-empty-actions button { display: inline-flex; align-items: center; gap: 7px; height: 32px; padding: 0 14px; border: 1px solid #dce2da; border-radius: 4px; cursor: pointer; }
  .empty-primary { color: #fff; border-color: #315841 !important; background: #315841; }
  .empty-primary:hover { background: #3a684d; }
  .empty-secondary { color: #43564a; background: #fff; }
  .empty-secondary:hover { background: #f0f3ee; }
  .editor-status { display: flex; height: 25px; flex: 0 0 25px; align-items: center; justify-content: flex-end; gap: 15px; padding: 0 13px; border-top: 1px solid #e9ede7; color: #8a958b; background: #f8faf6; font-size: 9px; }
  .editor-status span:first-child { overflow: hidden; max-width: 48%; margin-right: auto; text-overflow: ellipsis; white-space: nowrap; }
  .editor-status .clangd-status { overflow: hidden; max-width: 38%; text-overflow: ellipsis; white-space: nowrap; }
  .editor-status .clangd-status.ready { color: #4c7a55; }
  .editor-status .clangd-status.unavailable { color: #ae5c3e; }
  .case-sidebar { display: flex; min-height: 0; flex-direction: column; grid-column: 5; grid-row: 1; padding: 14px 13px 12px; background: #fcfdfb; }
  .panel-title-row { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .panel-title-row h2 { margin: 3px 0 0; color: #304137; font-size: 14px; font-weight: 680; }
  .panel-title-row h2 small { margin-left: 4px; color: #a0aaa0; font-size: 10px; font-weight: 550; }
  .modal-title-row { display: flex; align-items: center; justify-content: space-between; }
  .modal-close { color: #8a958a; }
  .test-manager-modal h2 { margin: 12px 0 16px; color: #2f4236; font-size: 18px; }
  .test-manager-body { display: grid; min-height: 0; flex: 1; grid-template-columns: 1.5fr 1fr; gap: 26px; overflow: hidden; }
  .test-manager-editor { display: flex; min-width: 0; min-height: 0; flex-direction: column; overflow: auto; }
  .test-manager-editor .case-textarea { min-height: 240px; }
  .test-manager-editor .expected-area { min-height: 190px; }
  .test-manager-list { display: flex; min-width: 0; min-height: 0; flex-direction: column; padding-left: 24px; border-left: 1px solid #e4e9e2; overflow: hidden; }
  .test-manager-list .panel-title-row { margin-bottom: 11px; }
  .test-manager-list .case-list { min-height: 0; flex: 1; }
  .test-manager-list .case-chip { min-height: 40px; }
  .run-case-button { display: grid; width: 29px; height: 29px; place-items: center; border: 1px solid #dce5da; border-radius: 5px; color: #416949; background: #eff5ec; cursor: pointer; }
  .run-case-button:hover { background: #e3eddf; }
  .case-name-row { display: flex; align-items: center; gap: 4px; margin-bottom: 10px; }
  .case-name { min-width: 0; flex: 1; padding: 5px 7px; border: 1px solid #e4e9e2; border-radius: 4px; outline: none; color: #45564a; background: #fff; font-size: 11px; }
  .case-name:focus, .case-textarea:focus { border-color: #93ad91; box-shadow: 0 0 0 2px #d9e6d6; }
  .remove-case { color: #a2aaa1; }
  .code-field-label { margin: 5px 0; color: #8a958a; font-size: 9px; font-weight: 750; }
  .case-textarea { width: 100%; min-height: 72px; flex: 1; resize: vertical; padding: 8px; border: 1px solid #e4e9e2; border-radius: 4px; outline: none; color: #425349; background: #f9fbf8; font-family: "Cascadia Code", Consolas, monospace; font-size: 11px; line-height: 1.55; tab-size: 4; }
  .expected-area { min-height: 62px; max-height: 40%; flex: .8; }
  .case-result-row { display: flex; min-height: 30px; align-items: center; gap: 8px; color: #879187; font-size: 10px; }
  .result-pill { padding: 3px 7px; border-radius: 3px; background: #edf4ea; font-size: 9px; font-weight: 800; }
  .result-placeholder { color: #9aa39a; }
  .console-panel { display: flex; min-width: 0; min-height: 0; flex-direction: column; grid-column: 3 / 6; grid-row: 3; overflow: hidden; border-top: 1px solid #e1e6df; background: #fbfcf9; }
  .splitter { z-index: 2; display: flex; align-items: center; justify-content: center; min-width: 0; min-height: 0; padding: 0; border: 0; appearance: none; background: transparent; touch-action: none; user-select: none; }
  .splitter span { flex: 0 0 auto; border-radius: 2px; background: #cdd6cc; transition: background-color .12s ease, width .12s ease, height .12s ease; }
  .splitter:hover span, .splitter:focus-visible span, .workbench.resizing .splitter span { background: #b96a45; }
  .splitter:focus-visible { outline: 2px solid #b96a45; outline-offset: -1px; }
  .splitter-sidebar { grid-column: 2; grid-row: 1 / 4; cursor: col-resize; }
  .splitter-sidebar span, .splitter-main span { width: 2px; height: 34px; }
  .splitter-main { grid-column: 4; grid-row: 1; cursor: col-resize; }
  .splitter-console { grid-column: 3 / 6; grid-row: 2; cursor: row-resize; }
  .splitter-console span { width: 34px; height: 2px; }
  .workbench.resizing, .workbench.resizing * { user-select: none; }
  .console-header { display: flex; min-height: 38px; align-items: stretch; justify-content: space-between; border-bottom: 1px solid #e8ece6; background: #f8faf6; }
  .console-tabs { display: flex; align-items: stretch; gap: 3px; padding-left: 9px; }
  .console-tabs button { display: inline-flex; align-items: center; gap: 6px; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: #829084; background: transparent; font-size: 10px; cursor: pointer; }
  .console-tabs button.selected { border-bottom-color: #52775c; color: #3e5d46; }
  .console-state { display: flex; align-items: center; gap: 7px; overflow: hidden; padding: 0 13px; color: #849084; font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
  .console-state.error { color: #ae5c3e; }
  .console-state.success { color: #4c7a55; }
  .working-indicator { width: 8px; height: 8px; border: 1px solid #8eaa87; border-top-color: transparent; border-radius: 50%; animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .output-columns, .diff-columns { display: grid; min-height: 0; flex: 1; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); overflow: hidden; }
  .output-block, .diff-columns > div { display: flex; min-width: 0; min-height: 0; flex-direction: column; overflow: hidden; padding: 9px 13px; }
  .output-block + .output-block, .diff-columns > div + div { border-left: 1px solid #e9ede7; }
  .output-label { flex: 0 0 auto; margin-bottom: 6px; color: #96a096; font-size: 9px; font-weight: 750; }
  .output-block pre, .diff-columns pre { flex: 1; }
  .output-block pre, .diff-columns pre, .compiler-output { overflow: auto; min-height: 0; margin: 0; color: #45564b; font-family: "Cascadia Code", Consolas, monospace; font-size: 11px; line-height: 1.5; white-space: pre-wrap; overflow-wrap: anywhere; }
  .stderr-block pre { color: #a65b42; }
  .compiler-output { min-height: 0; flex: 1; padding: 11px 14px; }
  .diff-columns { position: relative; padding-bottom: 23px; }
  .diff-note { position: absolute; right: 12px; bottom: 5px; margin: 0; color: #ae6547; font-size: 9px; }
  .diff-columns { grid-template-columns: minmax(0, 1fr) 7px minmax(0, 1fr) 7px minmax(0, 1fr); }
  .col-splitter { position: relative; cursor: col-resize; }
  .col-splitter::before { content: ""; position: absolute; top: 0; bottom: 0; left: 50%; width: 1px; background: #e9ede7; }
  .col-splitter span { position: relative; width: 2px; height: 34px; }
  .col-splitter.active span { background: #b96a45; }
  .diff-count { margin-left: 8px; color: #c4917b; font-weight: 650; }
  .actual-label { display: flex; align-items: center; }
  .diff-toggle { margin-left: auto; padding: 3px 8px; border: 1px solid #dfe6dc; border-radius: 4px; color: #7f8f83; background: transparent; font-size: 9px; font-weight: 650; cursor: pointer; }
  .diff-toggle:hover { border-color: #c9d6c6; background: #f3f7f1; }
  .diff-toggle.on { border-color: #e7c3b6; color: #b0735c; background: #fbeee9; }
  .diff-bad { border-radius: 2px; background: #fbdcd2; }
  .diff-missing { position: relative; display: inline-block; width: 0; height: 1.5em; vertical-align: top; cursor: help; }
  .diff-missing::after { content: ""; position: absolute; top: 0; bottom: 0; left: -1px; width: 2px; background: #eba08e; }
  .diff-ghost { color: #c9bab3; background: #fcf6f3; user-select: none; }
  .numbered .ln { position: relative; display: block; min-height: 1.5em; padding-left: calc(var(--gw, 2ch) + 12px); }
  .numbered .ln::before { content: attr(data-n); position: absolute; left: 0; width: var(--gw, 2ch); color: #b3bdb3; text-align: right; user-select: none; }
  .statusbar { display: flex; height: 25px; flex: 0 0 25px; align-items: center; justify-content: space-between; padding: 0 10px; border-radius: 3px; color: #7f8a80; font-size: 9px; }
  .status-project { display: flex; min-width: 0; align-items: center; gap: 7px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .statusbar b { margin-left: 4px; color: #596a5d; font-weight: 650; }
  .modal-backdrop { position: fixed; z-index: 5; inset: 0; display: grid; place-items: center; padding: 20px; background: #1e2d2470; backdrop-filter: blur(3px); }
  .project-modal { width: min(390px, 100%); padding: 24px; border: 1px solid #dce4d9; border-radius: 7px; background: #fbfcf9; box-shadow: 0 18px 50px #18231d40; animation: appear .18s ease-out both; }
  .project-modal::backdrop { background: transparent; }
  @keyframes appear { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
  .modal-icon { display: grid; width: 34px; height: 34px; place-items: center; border-radius: 5px; color: #315841; background: #eaf1e7; }
  .project-modal h2 { margin: 14px 0 5px; color: #2f4236; font-size: 18px; }
  .project-modal p { margin: 0 0 19px; color: #7d897e; font-size: 11px; line-height: 1.6; }
  .project-modal form { display: flex; flex-direction: column; gap: 7px; }
  .project-modal label { color: #657268; font-size: 10px; font-weight: 700; }
  .project-modal input { height: 36px; padding: 0 10px; border: 1px solid #dce4da; border-radius: 4px; outline: none; color: #34463a; background: white; }
  .project-modal input:focus { border-color: #86a282; box-shadow: 0 0 0 2px #dfeadd; }
  .test-manager-modal {position: relative;box-sizing: border-box;width: 880px;min-width: 0;max-width: calc(100% - 40px);height: 660px;max-height: calc(100% - 40px);margin: 0;}
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 11px; }
  .modal-actions button { min-height: 32px; padding: 0 11px; border: 1px solid #dce3da; border-radius: 4px; cursor: pointer; font-size: 10px; font-weight: 650; }
  .cancel-button { color: #5d6c61; background: #fff; }
  .confirm-button { border-color: #315841 !important; color: white; background: #315841; }
  @media (max-width: 980px) {
    .app-shell { min-height: 780px; }
    .actionbar { flex-wrap: wrap; flex: 0 0 auto; gap: 4px 12px; padding: 5px 12px; }
    .workbench { grid-template-columns: var(--sidebar-width) 6px minmax(280px, 1fr); grid-template-rows: minmax(240px, var(--editor-height)) 6px minmax(180px, var(--test-height)) 6px minmax(150px, var(--console-height)); }
    .sidebar { grid-column: 1; grid-row: 1 / 6; }
    .editor-panel { grid-column: 3; grid-row: 1; }
    .case-sidebar { grid-column: 3; grid-row: 3; border-top: 1px solid #e1e6df; border-left: 0; }
    .console-panel { grid-column: 3; grid-row: 5; }
    .splitter-sidebar { grid-column: 2; grid-row: 1 / 6; }
    .splitter-main { grid-column: 3; grid-row: 2; cursor: row-resize; }
    .splitter-main span { width: 34px; height: 2px; }
    .splitter-console { grid-column: 3; grid-row: 4; }
    .case-textarea { min-height: 45px; }
    .expected-area { min-height: 40px; }
    .test-manager-modal { width: min(94vw, 560px); height: auto; max-height: 92vh; padding: 22px; }
    .test-manager-body { grid-template-columns: 1fr; overflow: auto; }
    .test-manager-list { min-height: 160px; padding-left: 0; padding-top: 14px; border-left: 0; border-top: 1px solid #e4e9e2; }
    .test-manager-editor .case-textarea { min-height: 90px; }
    .test-manager-editor .expected-area { min-height: 70px; }
  }
  @media (max-width: 740px) {
    :global(html), :global(body) { min-width: 360px; }
    .app-shell { --pad-x: 7px; --pad-y: 7px; height: auto; min-height: 100vh; gap: 6px; }
    .actionbar { align-items: flex-start; flex-direction: column; gap: 4px; padding: 5px 8px; }
    .file-actions, .run-actions { width: 100%; justify-content: space-between; }
    .text-action { padding: 0 5px; font-size: 10px; }
    .run-actions { gap: 4px; }
    .timeout-field { padding: 0 4px; }
    .compile-button, .run-button, .test-button, .stop-button { gap: 4px; padding: 0 7px; font-size: 10px; }
    .workbench { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(100px, var(--sidebar-height)) 6px minmax(280px, var(--editor-height)) 6px minmax(240px, var(--test-height)) 6px minmax(180px, var(--console-height)); overflow: visible; }
    .sidebar { min-height: 0; max-height: none; grid-column: 1; grid-row: 1; border-right: 0; border-bottom: 1px solid #e1e6df; }
    .files-section { min-height: 90px; }
    .editor-panel { min-height: 0; grid-column: 1; grid-row: 3; border-right: 0; }
    .case-sidebar { min-height: 0; grid-column: 1; grid-row: 5; border-top: 1px solid #e1e6df; border-left: 0; }
    .console-panel { min-height: 0; grid-column: 1; grid-row: 7; border-top: 1px solid #e1e6df; }
    .splitter-sidebar { grid-column: 1; grid-row: 2; cursor: row-resize; }
    .splitter-sidebar span { width: 34px; height: 2px; }
    .splitter-main { grid-column: 1; grid-row: 4; }
    .splitter-console { grid-column: 1; grid-row: 6; }
    .statusbar { gap: 10px; }
    .status-project { max-width: 68%; }
  }
</style>