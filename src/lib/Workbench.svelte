<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, highlightActiveLineGutter, keymap, lineNumbers } from "@codemirror/view";
  import { bracketMatching, defaultHighlightStyle, indentOnInput, syntaxHighlighting } from "@codemirror/language";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { cpp } from "@codemirror/lang-cpp";
  import { searchKeymap } from "@codemirror/search";
  import localforage from "localforage";
  import { LanguageServerClient, languageServerWithTransport, type Transport } from "codemirror-languageserver";
  import { CircleCheck, CirclePlus, CircleX, Clock3, Code2, FileCode2, FolderOpen, FolderPlus, Play, Save, Search, Settings2, Square, Terminal, Trash2 } from "lucide-svelte";

  type ResultStatus = "AC" | "WA" | "RE" | "TLE";
  type ConsoleTab = "output" | "build" | "diff";
  type ResizeKind = "main" | "console";
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

  const starterCode = `#include <bits/stdc++.h>
using namespace std;

int main() {
    cin.tie(nullptr)->sync_with_stdio(false);
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
  let lspTransport: TauriLspTransport | undefined;
  let languageClient: LanguageServerClient | undefined;
  let lspSession: LspSessionInfo | undefined;
  let clangdStatus = $state("等待開啟 C++ 檔案");
  let toolchainsReady = $state(false);
  let toolchainStatus = $state("正在檢查 C++ 工具鏈...");
  let toolchainInitialization: Promise<ToolchainInfo>;
  let source = $state(starterCode);
  let projectPath = $state("");
  let filePaths = $state<string[]>([]);
  let activePath = $state("");
  let dirty = $state(false);
  let testCases = $state<TestCase[]>([{ id: "sample", name: "輸入輸出", input: "5\n", expectedOutput: "5\n" }]);
  let activeTestId = $state("sample");
  let activeTest = $derived(testCases.find((testCase) => testCase.id === activeTestId));
  let timeoutMs = $state(2000);
  let busy = $state("");
  let activeRunId = $state("");
  let cancelRequested = $state(false);
  let compilerOutput = $state("");
  let stdout = $state("");
  let stderr = $state("");
  let consoleTab = $state<ConsoleTab>("output");
  let notice = $state("準備就緒");
  let noticeTone = $state("neutral");
  let showNewProject = $state(false);
  let projectName = $state("");
  let testWidth = $state(280);
  let consoleHeight = $state(190);
  let editorHeight = $state(330);
  let testHeight = $state(210);
  let activeResize = $state<{ kind: ResizeKind; pointerId: number } | null>(null);
  let showTestManager = $state(false);
  let viewportMode = $state<"desktop" | "stacked" | "mobile">("desktop");

  onMount(() => {
    const updateViewportMode = () => {
      viewportMode = window.innerWidth <= 740 ? "mobile" : window.innerWidth <= 980 ? "stacked" : "desktop";
    };
    updateViewportMode();
    window.addEventListener("resize", updateViewportMode);
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
      if (typeof layout.testWidth === "number") testWidth = clamp(layout.testWidth, 235, 480);
      if (typeof layout.consoleHeight === "number") consoleHeight = clamp(layout.consoleHeight, 150, 420);
      if (typeof layout.editorHeight === "number") editorHeight = clamp(layout.editorHeight, 240, 560);
      if (typeof layout.testHeight === "number") testHeight = clamp(layout.testHeight, 180, 420);
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
          lspCompartment.of([]),
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
      window.removeEventListener("resize", updateViewportMode);
      closeLanguageServer();
      editorView?.destroy();
    };
  });

  function setEditorContent(content: string) {
    source = content;
    editorView?.dispatch({ changes: { from: 0, to: editorView.state.doc.length, insert: content } });
    dirty = false;
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
      localStorage.setItem(layoutStorageKey, JSON.stringify({ testWidth, consoleHeight, editorHeight, testHeight }));
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
    if (activeResize.kind === "main") {
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
  function resizeWithKeyboard(event: KeyboardEvent, kind: ResizeKind) {
    const step = event.shiftKey ? 32 : 12;
    if (kind === "main") {
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
        activePath = "";
        setEditorContent(starterCode);
        closeLanguageServer();
        setNotice("專案已開啟，新增一個 C++ 檔案開始撰寫");
      }
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function openSourceFile() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "C++ 原始碼", extensions: ["cpp", "cc", "cxx", "h", "hpp"] }] });
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
  function newSourceFile() {
    if (!confirmDiscardChanges()) return;
    activePath = "";
    setEditorContent(starterCode);
    closeLanguageServer();
    setNotice("新檔案尚未儲存");
  }
  async function saveCurrent(): Promise<boolean> {
    let path = activePath;
    if (!path) {
      try {
        const selected = await save({
          defaultPath: projectPath ? joinPath(projectPath, "main.cpp") : "main.cpp",
          filters: [{ name: "C++ 原始碼", extensions: ["cpp"] }]
        });
        if (!selected) return false;
        path = selected;
      } catch (error) { setNotice(String(error), "error"); return false; }
    }
    try {
      await invoke("save_source", { path, content: source });
      activePath = path;
      projectPath = path.replace(/[\\/][^\\/]+$/, "");
      dirty = false;
      await refreshFiles();
      await connectLanguageServer(path);
      setNotice("檔案已儲存", "success");
      return true;
    } catch (error) { setNotice(String(error), "error"); return false; }
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
    testCases = [...testCases, { id, name: `測資 ${testCases.length + 1}`, input: "", expectedOutput: "" }];
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

<div class="app-shell">
  <header class="topbar">
    <a class="brand" href="/" aria-label="IDE for Exam 首頁"><span class="brand-mark"><Code2 size={18} /></span><span class="brand-name">IDE<span>for</span>EXAM</span></a>
    <div class="project-heading"><span class="eyebrow">WORKSPACE</span><strong>{projectPath ? projectPath.split(/[\\/]/).at(-1) : "未開啟專案"}</strong>{#if dirty}<i class="dirty-dot" title="尚未儲存"></i>{/if}</div>
    <div class="top-actions">
      <button class="icon-button" title="建立專案" aria-label="建立專案" onclick={() => showNewProject = true}><FolderPlus size={17} /></button>
      <button class="icon-button" title="開啟專案資料夾" aria-label="開啟專案資料夾" onclick={openProject}><FolderOpen size={17} /></button>
      <span class="top-divider"></span>
      <button class="save-button" onclick={saveCurrent} disabled={busy !== ""}><Save size={15} />儲存</button>
    </div>
  </header>
  <div class="actionbar">
    <div class="file-actions">
      <button class="text-action" onclick={newSourceFile}><CirclePlus size={15} />新檔案</button>
      <button class="text-action" onclick={openSourceFile}><FolderOpen size={15} />開啟檔案</button>
      <button class="text-action" onclick={saveCurrent}><Save size={15} />儲存檔案</button>
    </div>
    <div class="run-actions">
      <label class="timeout-field" title="程式逾時上限"><Clock3 size={14} /><input type="number" min="100" max="300000" step="100" bind:value={timeoutMs} aria-label="執行逾時毫秒" /><span>ms</span></label>
      <button class="compile-button" onclick={compileCurrent} disabled={busy !== "" || !toolchainsReady}><Settings2 size={15} />編譯</button>
      <button class="run-button" onclick={runProgram} disabled={busy !== "" || !toolchainsReady}><Play size={15} fill="currentColor" />執行</button>
      <button class="test-button" onclick={() => runTests(true)} disabled={busy !== "" || !toolchainsReady}><CircleCheck size={15} />全部測試</button>
      {#if activeRunId && (busy === "run" || busy === "test")}<button class="stop-button" onclick={stopCurrentRun}><Square size={13} fill="currentColor" />停止</button>{/if}
    </div>
  </div>
  <main
    class="workbench"
    class:resizing={activeResize !== null}
    bind:this={workbenchElement}
    style={`--test-width:${testWidth}px;--console-height:${consoleHeight}px;--editor-height:${editorHeight}px;--test-height:${testHeight}px`}
  >
    <section class="editor-panel" bind:this={editorPanelElement}>
      <div class="editor-tabbar"><div class="active-file-tab"><FileCode2 size={15} /><span>{activePath ? relativeFile(activePath) : "untitled.cpp"}</span>{#if dirty}<i></i>{/if}</div><div class="editor-shortcut"><Search size={13} /><span>Ctrl F 搜尋</span></div></div>
      <div class="editor-host" bind:this={editorElement}></div>
      <div class="editor-status"><span>{activePath ? relativeFile(activePath) : "未儲存"}</span><span class:ready={clangdStatus === "clangd IntelliSense"} class:unavailable={clangdStatus.startsWith("IntelliSense 無法") || clangdStatus.startsWith("clangd:")} class="clangd-status" title={clangdStatus}>{clangdStatus}</span><span>C++17</span><span>UTF-8</span><span>LF</span></div>
    </section>
    <aside class="case-sidebar">
      <div class="panel-title-row"><div><span class="eyebrow">LOCAL JUDGE</span><h2>測試案例 <small>{testCases.length}</small></h2></div><button class="mini-icon" title="測資編輯主控台" aria-label="測資編輯主控台" onclick={openTestManager}><Settings2 size={15} /></button></div>
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
            <span class="case-index">#{String(index + 1).padStart(2, "0")}</span>
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
      {:else if activeTest}<div class="diff-columns"><div><div class="output-label">EXPECTED</div><pre>{activeTest.expectedOutput}</pre></div><div><div class="output-label">ACTUAL</div><pre>{activeTest.actualOutput ?? "尚未執行此測資"}</pre></div>{#if activeTest.firstDifference !== undefined && activeTest.firstDifference !== null}<p class="diff-note">第一個差異位於第 {activeTest.firstDifference + 1} 個字元</p>{/if}</div>
      {:else}<pre class="compiler-output">請先建立測資。</pre>{/if}
    </section>
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
  <h2 id="test-manager-title">測資編輯主控台</h2>
  <div class="test-manager-body">
    <div class="test-manager-editor">
      {#if activeTest}
        <div class="case-name-row">
          <input class="case-name" value={activeTest.name} aria-label="測資名稱" onchange={(event) => updateTest(activeTest.id, "name", event.currentTarget.value)} />
          <button class="run-case-button" onclick={() => runTests(false)} disabled={busy !== ""} title="執行目前測資"><Play size={14} fill="currentColor" /></button>
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
      <div class="panel-title-row"><h2>測試案例 <small>{testCases.length}</small></h2><button class="mini-icon" title="新增測資" aria-label="新增測資" onclick={addTestCase}><CirclePlus size={15} /></button></div>
      {#if testCases.length}
        <ul class="case-list">{#each testCases as testCase, index (testCase.id)}<li class="case-row">
          <button
            class="case-chip"
            class:case-ac={testCase.status === "AC"}
            class:case-wa={testCase.status === "WA"}
            class:case-other={testCase.status !== "AC" && testCase.status !== "WA"}
            class:active={activeTestId === testCase.id}
            onclick={() => activeTestId = testCase.id}
            title={`編輯 ${testCase.name}`}
          >
            <span class="case-index">#{String(index + 1).padStart(2, "0")}</span>
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
  .app-shell { display: flex; flex-direction: column; width: 100%; height: 100vh; min-height: 620px; padding: 12px 14px 8px; gap: 8px; background: radial-gradient(ellipse at 4% 0%, #f5f7f2 0%, #e8ece7 49%, #e4e9e3 100%); }
  .topbar, .actionbar, .workbench, .statusbar { border: 1px solid #d6ddd5; background: #fbfcf9; }
  .topbar { display: flex; align-items: center; height: 56px; flex: 0 0 56px; padding: 0 16px; border-radius: 7px 7px 3px 3px; box-shadow: 0 2px 8px #2033270b; }
  .brand { display: flex; align-items: center; gap: 9px; width: 216px; text-decoration: none; color: inherit; }
  .brand-mark { display: grid; place-items: center; width: 31px; height: 31px; border-radius: 6px; color: #fff; background: #315841; }
  .brand-name { color: #26382d; font-size: 13px; font-weight: 760; }
  .brand-name span { color: #b9623d; font-family: Georgia, serif; font-weight: 500; }
  .project-heading { display: flex; min-width: 0; align-items: center; gap: 9px; padding-left: 19px; border-left: 1px solid #e0e5df; }
  .eyebrow { display: block; color: #879287; font-size: 9px; font-weight: 750; }
  .project-heading strong { overflow: hidden; max-width: 42vw; color: #34443a; font-size: 12px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
  .dirty-dot, .file-dirty { width: 7px; height: 7px; flex: 0 0 7px; border-radius: 50%; background: #c56b40; }
  .top-actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
  .icon-button, .mini-icon { display: grid; place-items: center; border: 0; background: transparent; color: #68776d; cursor: pointer; }
  .icon-button { width: 32px; height: 32px; border-radius: 5px; }
  .icon-button:hover, .mini-icon:hover { color: #315841; background: #eef2ec; }
  .top-divider { width: 1px; height: 22px; margin: 0 3px; background: #e0e5df; }
  .save-button, .text-action, .compile-button, .run-button, .test-button, .stop-button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; border: 1px solid transparent; border-radius: 4px; cursor: pointer; font-size: 11px; font-weight: 650; }
  .save-button { height: 32px; padding: 0 12px; border-color: #dce2da; color: #43564a; background: #fff; }
  .save-button:hover, .text-action:hover { background: #f0f3ee; }
  .actionbar { display: flex; align-items: center; justify-content: space-between; min-height: 44px; flex: 0 0 44px; padding: 0 12px; border-radius: 3px; }
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
  .workbench { display: grid; min-height: 0; flex: 1; grid-template-columns: minmax(300px, 1fr) 6px var(--test-width); grid-template-rows: minmax(240px, 1fr) 6px var(--console-height); overflow: hidden; border-radius: 3px; box-shadow: 0 5px 18px #2033270b; }
  .mini-icon { width: 25px; height: 25px; border-radius: 4px; }
  .case-list { display: flex; min-height: 0; flex-direction: column; gap: 6px; margin: 0; padding: 0; overflow: auto; list-style: none; }
  .case-row { display: flex; align-items: center; gap: 6px; }
  .case-chip { display: flex; flex: 1; min-width: 0; align-items: center; justify-content: space-between; min-height: 34px; padding: 0 10px; border: 1.5px solid #e4e9e2; border-radius: 5px; color: #45564a; background: #fff; text-align: left; cursor: pointer; }
  .case-chip:hover { background: #f4f7f2; }
  .case-chip.active { background: #eef4ec; box-shadow: 0 0 0 1.5px #9ab89a inset; }
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
  .editor-panel { display: flex; min-width: 0; min-height: 0; flex-direction: column; grid-column: 1; grid-row: 1; border-right: 1px solid #e1e6df; }
  .editor-tabbar { display: flex; height: 39px; flex: 0 0 39px; align-items: stretch; justify-content: space-between; border-bottom: 1px solid #e5e9e3; background: #f8faf6; }
  .active-file-tab { display: flex; min-width: 0; align-items: center; gap: 8px; padding: 0 13px; border-bottom: 2px solid #52775c; color: #3f5446; font-size: 11px; }
  .active-file-tab span { overflow: hidden; max-width: 220px; text-overflow: ellipsis; white-space: nowrap; }
  .active-file-tab :global(svg) { flex: 0 0 auto; color: #758e76; }
  .active-file-tab i { width: 6px; height: 6px; border-radius: 50%; background: #c56b40; }
  .editor-shortcut { display: flex; align-items: center; gap: 5px; padding: 0 12px; color: #9aa39a; font-size: 10px; }
  .editor-host { min-height: 0; flex: 1; overflow: hidden; background: #fbfcf9; }
  .editor-status { display: flex; height: 25px; flex: 0 0 25px; align-items: center; justify-content: flex-end; gap: 15px; padding: 0 13px; border-top: 1px solid #e9ede7; color: #8a958b; background: #f8faf6; font-size: 9px; }
  .editor-status span:first-child { overflow: hidden; max-width: 48%; margin-right: auto; text-overflow: ellipsis; white-space: nowrap; }
  .editor-status .clangd-status { overflow: hidden; max-width: 38%; text-overflow: ellipsis; white-space: nowrap; }
  .editor-status .clangd-status.ready { color: #4c7a55; }
  .editor-status .clangd-status.unavailable { color: #ae5c3e; }
  .case-sidebar { display: flex; min-height: 0; flex-direction: column; grid-column: 3; grid-row: 1; padding: 14px 13px 12px; background: #fcfdfb; }
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
  .console-panel { display: flex; min-width: 0; min-height: 0; flex-direction: column; grid-column: 1 / 4; grid-row: 3; border-top: 1px solid #e1e6df; background: #fbfcf9; }
  .splitter { z-index: 2; display: flex; align-items: center; justify-content: center; min-width: 0; min-height: 0; padding: 0; border: 0; appearance: none; background: transparent; touch-action: none; user-select: none; }
  .splitter span { flex: 0 0 auto; border-radius: 2px; background: #cdd6cc; transition: background-color .12s ease, width .12s ease, height .12s ease; }
  .splitter:hover span, .splitter:focus-visible span, .workbench.resizing .splitter span { background: #b96a45; }
  .splitter:focus-visible { outline: 2px solid #b96a45; outline-offset: -1px; }
  .splitter-main span { width: 2px; height: 34px; }
  .splitter-main { grid-column: 2; grid-row: 1; cursor: col-resize; }
  .splitter-console { grid-column: 1 / 4; grid-row: 2; cursor: row-resize; }
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
  .output-columns, .diff-columns { display: grid; min-height: 0; flex: 1; grid-template-columns: 1fr 1fr; }
  .output-block, .diff-columns > div { min-width: 0; padding: 9px 13px; }
  .output-block + .output-block, .diff-columns > div + div { border-left: 1px solid #e9ede7; }
  .output-label { margin-bottom: 6px; color: #96a096; font-size: 9px; font-weight: 750; }
  .output-block pre, .diff-columns pre, .compiler-output { overflow: auto; max-height: calc(100% - 17px); margin: 0; color: #45564b; font-family: "Cascadia Code", Consolas, monospace; font-size: 11px; line-height: 1.5; white-space: pre-wrap; overflow-wrap: anywhere; }
  .stderr-block pre { color: #a65b42; }
  .compiler-output { min-height: 0; flex: 1; padding: 11px 14px; }
  .diff-columns { position: relative; padding-bottom: 23px; }
  .diff-note { position: absolute; right: 12px; bottom: 5px; margin: 0; color: #ae6547; font-size: 9px; }
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
    .workbench { grid-template-columns: minmax(280px, 1fr); grid-template-rows: minmax(240px, var(--editor-height)) 6px minmax(180px, var(--test-height)) 6px minmax(150px, var(--console-height)); }
    .editor-panel { grid-column: 1; grid-row: 1; }
    .case-sidebar { grid-column: 1; grid-row: 3; border-top: 1px solid #e1e6df; border-left: 0; }
    .console-panel { grid-column: 1; grid-row: 5; }
    .splitter-main { grid-column: 1; grid-row: 2; cursor: row-resize; }
    .splitter-main span { width: 34px; height: 2px; }
    .splitter-console { grid-column: 1; grid-row: 4; }
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
    .app-shell { height: auto; min-height: 100vh; padding: 7px; gap: 6px; }
    .topbar { padding: 0 9px; }
    .brand { width: auto; margin-right: 12px; }
    .brand-name { display: none; }
    .project-heading { padding-left: 10px; }
    .project-heading .eyebrow { display: none; }
    .project-heading strong { max-width: 32vw; }
    .top-actions { gap: 2px; }
    .actionbar { align-items: flex-start; flex-direction: column; gap: 4px; padding: 5px 8px; }
    .file-actions, .run-actions { width: 100%; justify-content: space-between; }
    .text-action { padding: 0 5px; font-size: 10px; }
    .run-actions { gap: 4px; }
    .timeout-field { padding: 0 4px; }
    .compile-button, .run-button, .test-button, .stop-button { gap: 4px; padding: 0 7px; font-size: 10px; }
    .workbench { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(280px, var(--editor-height)) 6px minmax(240px, var(--test-height)) 6px minmax(180px, var(--console-height)); overflow: visible; }
    .editor-panel { min-height: 0; grid-column: 1; grid-row: 1; border-right: 0; }
    .case-sidebar { min-height: 0; grid-column: 1; grid-row: 3; border-top: 1px solid #e1e6df; border-left: 0; }
    .console-panel { min-height: 0; grid-column: 1; grid-row: 5; border-top: 1px solid #e1e6df; }
    .splitter-main { grid-column: 1; grid-row: 2; }
    .splitter-console { grid-column: 1; grid-row: 4; }
    .statusbar { gap: 10px; }
    .status-project { max-width: 68%; }
  }
</style>