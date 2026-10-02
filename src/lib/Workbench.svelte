<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, highlightActiveLineGutter, keymap, lineNumbers } from "@codemirror/view";
  import { bracketMatching, defaultHighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from "@codemirror/language";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { cpp } from "@codemirror/lang-cpp";
  import { closeSearchPanel, getSearchQuery, search, searchKeymap, setSearchQuery } from "@codemirror/search";
  import localforage from "localforage";
  import { LanguageServerClient, languageServerWithTransport } from "codemirror-languageserver";
  import { CircleCheck, CirclePlus, CircleX, Clock3, Code2, Copy, FileCode2, FolderOpen, FolderPlus, Minus, Pencil, Play, Save, Search, Settings2, Square, Terminal, Trash2, X } from "lucide-svelte";

  import "./workbench/workbench.css";
  import type { CompareResult, CompileResult, ConsoleTab, EditorTab, LspSessionInfo, ResizeKind, ResultStatus, RunResult, TestCase, ToolchainInfo } from "./workbench/types";
  import { layoutStorageKey, starterCode, storageKey } from "./workbench/constants";
  import { clamp, joinPath } from "./workbench/utils";
  import { buildDiff, gutterStyle, numberedLines } from "./workbench/diff";
  import { TauriLspTransport } from "./workbench/lsp-transport";
  import { createSearchPanel } from "./workbench/search-panel";
  import { autoExpandBrace, editableExtensions } from "./workbench/editor-extensions";
  import { editorTheme } from "./workbench/editor-theme";


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
  let projectPath = $state("");
  let filePaths = $state<string[]>([]);
  // 檔案分頁：tabs 只放會影響畫面的資訊（路徑、是否未儲存）；
  // 每個分頁的 CodeMirror 狀態（文件、復原紀錄、游標）與捲動位置放在非響應式的 Map，切換時整份換上。
  let tabs = $state<EditorTab[]>([]);
  let activePath = $state("");
  let hasFile = $derived(activePath !== "");
  let tabStripElement = $state<HTMLElement | undefined>();
  const tabStates = new Map<string, EditorState>();
  const tabScroll = new Map<string, number>();
  let testCases = $state<TestCase[]>([{ id: "sample", name: "", input: "5\n", expectedOutput: "5\n" }]);
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
  let showDiffMarks = $state(false);
  let notice = $state("準備就緒");
  let noticeTone = $state("neutral");
  // 介面模式（簡易狀態機）：同一時間只會處於其中一個模式。
  // 擴充方式：在 UiMode 加入新名稱，再視需要在 uiModeHooks 補上進入 / 離開該模式時要做的事。
  type UiMode = "code" | "tests" | "newProject";
  const uiModeHooks: Partial<Record<UiMode, { enter?: () => void; leave?: () => void }>> = {
    // 離開「編輯程式」時收起 Ctrl+F 搜尋框（查詢內容仍保留，回來再按 Ctrl+F 即可）。
    code: { leave: () => { if (editorView) closeSearchPanel(editorView); } }
  };
  let uiMode = $state<UiMode>("code");
  let showNewProject = $derived(uiMode === "newProject");
  let showTestManager = $derived(uiMode === "tests");
  function enterMode(next: UiMode) {
    if (uiMode === next) return;
    uiModeHooks[uiMode]?.leave?.();
    uiMode = next;
    uiModeHooks[next]?.enter?.();
  }
  // 只有目前正處於該模式時才回到「編輯程式」，避免誤關其他模式。
  function exitMode(mode: UiMode) {
    if (uiMode === mode) enterMode("code");
  }
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

    editorView = new EditorView({ state: createEditorState("", false), parent: editorElement });

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

  function minimizeWindow() { void appWindow?.minimize(); }
  function toggleMaximizeWindow() { void appWindow?.toggleMaximize(); }
  function closeWindow() {
    if (!confirmDiscardChanges()) return;
    void appWindow?.close();
  }
  // 每個分頁都用同一組 extensions 建立獨立的 EditorState；沒有開啟任何檔案時用唯讀的空白狀態。
  function createEditorState(doc: string, editable: boolean) {
    return EditorState.create({
      doc,
      extensions: [
        lineNumbers(), highlightActiveLineGutter(), history(), indentOnInput(), bracketMatching(), cpp(),
        search({ top: true, createPanel: createSearchPanel }),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
        indentUnit.of("    "),
        EditorView.inputHandler.of(autoExpandBrace),
        EditorState.tabSize.of(4),
        lspCompartment.of([]),
        editableCompartment.of(editableExtensions(editable)),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && activePath) setTabDirty(activePath, true);
        }),
        EditorView.lineWrapping,
        editorTheme
      ]
    });
  }
  // 整份換上另一個分頁的狀態；搜尋字串（Ctrl+F 的查詢）會跟著帶過去，面板則收起。
  function swapEditorState(next: EditorState, scrollTop = 0) {
    const view = editorView;
    if (!view) return;
    const query = getSearchQuery(view.state);
    view.setState(next);
    if (query.search) view.dispatch({ effects: setSearchQuery.of(query) });
    requestAnimationFrame(() => { view.scrollDOM.scrollTop = scrollTop; });
  }
  // 離開目前分頁前，把編輯器狀態存回 Map（先卸掉 LSP 外掛，回來時再由 connectLanguageServer 掛上）。
  function stashActiveTab() {
    const view = editorView;
    if (!view || !activePath) return;
    closeSearchPanel(view);
    view.dispatch({ effects: lspCompartment.reconfigure([]) });
    tabStates.set(activePath, view.state);
    tabScroll.set(activePath, view.scrollDOM.scrollTop);
  }
  function clearEditor() {
    activePath = "";
    swapEditorState(createEditorState("", false));
    closeLanguageServer();
  }
  function setTabDirty(path: string, value: boolean) {
    const tab = tabs.find((item) => item.path === path);
    if (tab && tab.dirty !== value) tab.dirty = value;
  }
  function baseName(path: string) { return path.split(/[\\/]/).at(-1) ?? path; }
  // Windows 路徑不分大小寫、斜線方向可能不同，比較時先正規化。
  function normalizePath(path: string) { return path.replace(/\//g, "\\").toLowerCase(); }
  function samePath(a: string, b: string) { return normalizePath(a) === normalizePath(b); }
  function findTab(path: string) { return tabs.find((tab) => samePath(tab.path, path)); }
  function isInsideProject(path: string) {
    if (!projectPath || path.length <= projectPath.length) return false;
    return samePath(path.slice(0, projectPath.length), projectPath) && /[\\/]/.test(path[projectPath.length]);
  }
  // 分頁標籤：預設只顯示檔名；有同名檔案同時開啟時加上上層資料夾以區分。
  function tabLabel(path: string) {
    const name = baseName(path);
    const clash = tabs.some((tab) => tab.path !== path && baseName(tab.path).toLowerCase() === name.toLowerCase());
    if (!clash) return name;
    const parts = path.split(/[\\/]/);
    return parts.length > 1 ? `${parts.at(-2)}/${name}` : name;
  }
  function isPathDirty(path: string) { return findTab(path)?.dirty ?? false; }
  function tabContent(path: string) {
    if (path === activePath && editorView) return editorView.state.doc.toString();
    return tabStates.get(path)?.doc.toString() ?? "";
  }
  function setNotice(message: string, tone = "neutral") { notice = message; noticeTone = tone; }
  // 切換分頁可能很快，連線動作排成佇列依序執行，避免同時啟動多個 clangd；已不是目前分頁的請求直接略過。
  let lspConnectChain: Promise<void> = Promise.resolve();
  function connectLanguageServer(path: string) {
    const run = lspConnectChain.then(() => doConnectLanguageServer(path));
    lspConnectChain = run.catch(() => {});
    return run;
  }
  async function doConnectLanguageServer(path: string) {
    if (activePath !== path) return;
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
    if (activePath !== path) return;
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
    const unsaved = tabs.filter((tab) => tab.dirty);
    if (!unsaved.length) return true;
    const names = unsaved.map((tab) => tabLabel(tab.path)).join("、");
    return window.confirm(unsaved.length === 1 ? `「${names}」有尚未儲存的變更，要捨棄嗎？` : `有 ${unsaved.length} 個檔案尚未儲存（${names}），要捨棄嗎？`);
  }
  function relativeFile(path: string) {
    return isInsideProject(path) ? path.slice(projectPath.length).replace(/^[\\/]/, "") : baseName(path);
  }
  async function refreshFiles(path = projectPath) {
    if (path) filePaths = await invoke<string[]>("list_source_files", { projectPath: path });
  }
  // 開啟檔案：已開啟就切過去，否則讀檔後新增一個分頁。
  async function openFile(path: string) {
    const existing = findTab(path);
    if (existing) { await activateTab(existing.path); return; }
    try {
      const contents = await invoke<string>("read_source", { path });
      stashActiveTab();
      tabStates.set(path, createEditorState(contents, true));
      tabs = [...tabs, { path, dirty: false }];
      activePath = path;
      swapEditorState(tabStates.get(path)!);
      if (uiMode === "code") editorView?.focus();
      setNotice(`已開啟 ${relativeFile(path)}`);
      await connectLanguageServer(path);
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function activateTab(path: string) {
    if (path === activePath) { if (uiMode === "code") editorView?.focus(); return; }
    const state = tabStates.get(path);
    if (!state) return;
    stashActiveTab();
    activePath = path;
    swapEditorState(state, tabScroll.get(path) ?? 0);
    if (uiMode === "code") editorView?.focus();
    await connectLanguageServer(path);
  }
  function cycleTab(direction: 1 | -1) {
    if (tabs.length < 2) return;
    const index = tabs.findIndex((tab) => tab.path === activePath);
    void activateTab(tabs[(index + direction + tabs.length) % tabs.length].path);
  }
  function closeTab(path: string) {
    const index = tabs.findIndex((tab) => tab.path === path);
    if (index < 0) return;
    if (tabs[index].dirty && !window.confirm(`「${tabLabel(path)}」有尚未儲存的變更，要捨棄嗎？`)) return;
    const wasActive = path === activePath;
    tabs = tabs.filter((tab) => tab.path !== path);
    tabStates.delete(path);
    tabScroll.delete(path);
    if (!wasActive) return;
    const next = tabs[Math.min(index, tabs.length - 1)];
    if (!next) { clearEditor(); return; }
    activePath = next.path;
    swapEditorState(tabStates.get(next.path)!, tabScroll.get(next.path) ?? 0);
    if (uiMode === "code") editorView?.focus();
    void connectLanguageServer(next.path);
  }
  function closeAllTabs() {
    tabs = [];
    tabStates.clear();
    tabScroll.clear();
    clearEditor();
  }
  function scrollTabs(event: WheelEvent) {
    if (!tabStripElement || Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
    tabStripElement.scrollLeft += event.deltaY;
  }
  // 目前分頁變動或新增分頁時，把作用中的分頁捲到可見範圍。
  $effect(() => {
    const path = activePath;
    void tabs.length;
    if (!tabStripElement || !path) return;
    void tick().then(() => tabStripElement?.querySelector<HTMLElement>(".editor-tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" }));
  });
  async function openProject() {
    if (!confirmDiscardChanges()) return;
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== "string") return;
      closeAllTabs();
      projectPath = selected;
      await refreshFiles(selected);
      if (filePaths.length) await openFile(filePaths[0]);
      else setNotice("專案已開啟，新增一個 C++ 檔案開始撰寫");
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function openSourceFile() {
    try {
      const selected = await open({ multiple: false, defaultPath: projectPath || undefined, filters: [{ name: "C++ 原始碼", extensions: ["cpp", "cc", "cxx", "h", "hpp"] }] });
      if (typeof selected !== "string") return;
      // 檔案已在目前專案內就保留專案資料夾；否則以該檔案所在資料夾作為專案。
      if (!isInsideProject(selected)) projectPath = selected.replace(/[\\/][^\\/]+$/, "");
      await openFile(selected);
      await refreshFiles();
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function createProject(event: SubmitEvent) {
    event.preventDefault();
    if (!projectName.trim()) return;
    try {
      const parent = await open({ directory: true, multiple: false });
      if (typeof parent !== "string") return;
      if (!confirmDiscardChanges()) return;
      closeAllTabs();
      projectPath = await invoke<string>("create_project", { parentPath: parent, name: projectName.trim() });
      projectName = "";
      exitMode("newProject");
      await refreshFiles();
      await openFile(joinPath(projectPath, "main.cpp"));
      setNotice("專案已建立", "success");
    } catch (error) { setNotice(String(error), "error"); }
  }
  async function newSourceFile() {
    try {
      const selected = await save({
        title: "新增 C++ 檔案",
        defaultPath: projectPath ? joinPath(projectPath, "main.cpp") : "main.cpp",
        filters: [{ name: "C++ 原始碼", extensions: ["cpp"] }]
      });
      if (!selected) { setNotice("已取消新增檔案"); return; }
      const path = /\.(cpp|cc|cxx|h|hpp)$/i.test(selected) ? selected : `${selected}.cpp`;
      await invoke("save_source", { path, content: starterCode });
      if (!projectPath || !isInsideProject(path)) projectPath = path.replace(/[\\/][^\\/]+$/, "");
      await refreshFiles();
      await openFile(path);
      setNotice(`已建立 ${relativeFile(path)}`, "success");
    } catch (error) { setNotice(String(error), "error"); }
  }
  // 寫入單一分頁；寫入期間若又有修改，維持「未儲存」狀態。
  async function writeTab(path: string) {
    const content = tabContent(path);
    await invoke("save_source", { path, content });
    if (tabContent(path) === content) setTabDirty(path, false);
  }
  async function saveCurrent(): Promise<boolean> {
    const path = activePath;
    if (!path) { setNotice("請先新增或開啟檔案", "error"); return false; }
    try {
      await writeTab(path);
      await refreshFiles();
      await connectLanguageServer(path);
      setNotice("檔案已儲存", "success");
      return true;
    } catch (error) { setNotice(String(error), "error"); return false; }
  }
  async function saveOtherDirtyTabs(): Promise<boolean> {
    for (const tab of tabs.filter((item) => item.dirty && item.path !== activePath)) {
      try { await writeTab(tab.path); }
      catch (error) { setNotice(`${tabLabel(tab.path)} 儲存失敗：${String(error)}`, "error"); return false; }
    }
    return true;
  }
  async function saveAllTabs() {
    const targets = tabs.filter((tab) => tab.dirty);
    if (!targets.length) { setNotice("沒有需要儲存的檔案"); return; }
    for (const tab of targets) {
      try { await writeTab(tab.path); }
      catch (error) { setNotice(`${tabLabel(tab.path)} 儲存失敗：${String(error)}`, "error"); return; }
    }
    await refreshFiles();
    setNotice(`已儲存 ${targets.length} 個檔案`, "success");
  }
  // 編譯前一併存下其他分頁（被 #include 的檔案必須是最新內容）。
  async function saveForBuild() { return (await saveOtherDirtyTabs()) && (await saveCurrent()); }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const mod = event.ctrlKey || event.metaKey;
    if (!mod || event.altKey) return;
    const key = event.key.toLowerCase();

    if (key === "s") {
      event.preventDefault();
      if (busy !== "" || event.repeat) return;
      if (event.shiftKey) void saveAllTabs();
      else if (hasFile) void saveCurrent();
      return;
    }
    if (uiMode !== "code") return;
    if (key === "w" && !event.shiftKey) {
      event.preventDefault();
      if (activePath && !event.repeat) closeTab(activePath);
    } else if (event.key === "Tab" || (!event.shiftKey && (event.key === "PageUp" || event.key === "PageDown"))) {
      event.preventDefault();
      cycleTab(event.key === "PageUp" || (event.key === "Tab" && event.shiftKey) ? -1 : 1);
    }
  }
  async function compileCurrent(): Promise<boolean> {
    const path = activePath;
    if (!toolchainsReady) { setNotice(toolchainStatus, "error"); return false; }
    if (!(await saveForBuild())) return false;
    busy = "compile";
    compilerOutput = "正在呼叫 G++...";
    consoleTab = "build";
    try {
      const result = await invoke<CompileResult>("compile_source", { path });
      compilerOutput = result.output || "Build Successful";
      setNotice(result.success ? "Build Successful" : "Build Failed", result.success ? "success" : "error");
      return result.success;
    } catch (error) { compilerOutput = String(error); setNotice(String(error), "error"); return false; }
    finally { busy = ""; }
  }
  async function runProgram() {
    const path = activePath;
    if (!(await compileCurrent())) return;
    busy = "run";
    consoleTab = "output";
    const runId = crypto.randomUUID();
    activeRunId = runId;
    cancelRequested = false;
    try {
      await invoke("register_run", { runId });
      const result = await invoke<RunResult>("run_source", { runId, path, input: activeTest?.input ?? "", timeoutMs });
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
    enterMode("tests");
  }
  function removeTestCase(id: string) {
    if (testCases.length === 1) return;
    testCases = testCases.filter((testCase) => testCase.id !== id);
    if (activeTestId === id) activeTestId = testCases[0].id;
    persistTests();
  }
  function openTestManager() {
    enterMode("tests");
  }
  function closeTestManager() {
    exitMode("tests");
  }
  function updateTest(id: string, field: "name" | "input" | "expectedOutput", value: string) {
    testCases = testCases.map((testCase) => testCase.id === id ? { ...testCase, [field]: value } : testCase);
    persistTests();
  }
  async function runTests(all: boolean) {
    const path = activePath;
    if (!toolchainsReady) { setNotice(toolchainStatus, "error"); return; }
    if (!(await saveForBuild())) return;
    busy = "test";
    compilerOutput = "正在編譯測試程式...";
    consoleTab = "build";
    let runId = "";
    try {
      const compiled = await invoke<CompileResult>("compile_source", { path });
      compilerOutput = compiled.output || "Build Successful";
      if (!compiled.success) { setNotice("Build Failed，測資未執行", "error"); return; }
      const casesToRun = all ? [...testCases] : testCases.filter((testCase) => testCase.id === activeTestId);
      runId = crypto.randomUUID();
      activeRunId = runId;
      cancelRequested = false;
      await invoke("register_run", { runId });
      for (const testCase of casesToRun) {
        if (cancelRequested) break;
        const result = await invoke<RunResult>("run_source", { runId, path, input: testCase.input, timeoutMs });
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
      <button class="text-action" onclick={() => enterMode("newProject")}><FolderPlus size={15} />建立專案</button>
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
        {#if filePaths.length}<ul class="file-list">{#each filePaths as file (file)}<li><button class:active={samePath(file, activePath)} class:opened={!!findTab(file)} class="file-item" onclick={() => openFile(file)}><FileCode2 size={15} /><span>{relativeFile(file)}</span>{#if isPathDirty(file)}<i class="file-dirty"></i>{/if}</button></li>{/each}</ul>
        {:else}<p class="empty-note">開啟資料夾以瀏覽來源檔</p>{/if}
      </section>
    </aside>
    <section class="editor-panel" bind:this={editorPanelElement}>
      <div class="editor-tabbar">
        <div class="editor-tabs" role="tablist" aria-label="已開啟的檔案" bind:this={tabStripElement} onwheel={scrollTabs}>
          {#each tabs as tab (tab.path)}
            <div class="editor-tab" class:active={tab.path === activePath} class:dirty={tab.dirty}>
              <button type="button" class="tab-main" role="tab" aria-selected={tab.path === activePath} title={tab.path}
                onclick={() => activateTab(tab.path)}
                onmousedown={(event) => { if (event.button === 1) event.preventDefault(); }}
                onauxclick={(event) => { if (event.button === 1) { event.preventDefault(); closeTab(tab.path); } }}
              ><FileCode2 size={14} /><span>{tabLabel(tab.path)}</span></button>
              <button type="button" class="tab-close" title="關閉 (Ctrl+W)" aria-label={`關閉 ${tabLabel(tab.path)}`} onclick={() => closeTab(tab.path)}><i class="tab-dirty-dot"></i><X size={13} /></button>
            </div>
          {:else}
            <div class="editor-tab empty-tab active"><span class="tab-main"><FileCode2 size={14} /><span>尚未開啟檔案</span></span></div>
          {/each}
        </div>
        <div class="editor-shortcut"><Search size={13} /><span>Ctrl F 搜尋</span></div>
      </div>
      <div class="editor-wrap" inert={uiMode !== "code"}>
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
      <div class="panel-title-row"><div><span class="eyebrow">LOCAL JUDGE</span><h2>測資列表 <small>len: {testCases.length}</small></h2></div><button class="case-editor-button" title="開啟測資編輯器" aria-label="開啟測資編輯器" onclick={openTestManager}><Pencil size={13} />編輯測資</button></div>
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
  <form onsubmit={createProject}><label for="project-name">專案名稱</label><input id="project-name" bind:value={projectName} placeholder="例如：apcs-practice" /><div class="modal-actions"><button type="button" class="cancel-button" onclick={() => exitMode("newProject")}>取消</button><button type="submit" class="confirm-button" disabled={!projectName.trim()}>選擇位置並建立</button></div></form>
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
