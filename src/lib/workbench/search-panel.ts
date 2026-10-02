import { EditorView, runScopeHandlers, type Panel, type ViewUpdate } from "@codemirror/view";
import { SearchQuery, closeSearchPanel, findNext, findPrevious, getSearchQuery, replaceAll, replaceNext, setSearchQuery } from "@codemirror/search";

// Ctrl+F 搜尋面板：仿 VS Code 的右上角浮動樣式（尋找 / 取代、區分大小寫、全字比對、規則運算式）。
let searchReplaceOpen = false;
const svgIcon = (paths: string) => `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
const chevronIcon = svgIcon('<path d="m9 18 6-6-6-6"/>');
const arrowUpIcon = svgIcon('<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>');
const arrowDownIcon = svgIcon('<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>');
const closeIcon = svgIcon('<path d="M18 6 6 18"/><path d="m6 6 12 12"/>');

export function createSearchPanel(view: EditorView): Panel {
  let query = getSearchQuery(view.state);

  const makeButton = (className: string, html: string, title: string, onClick: () => void) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = className;
    button.title = title;
    button.setAttribute("aria-label", title);
    button.innerHTML = html;
    // 避免點擊按鈕時輸入框失去焦點。
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", onClick);
    return button;
  };
  const makeInput = (className: string, placeholder: string) => {
    const input = document.createElement("input");
    input.type = "text";
    input.className = className;
    input.placeholder = placeholder;
    input.setAttribute("aria-label", placeholder);
    input.setAttribute("spellcheck", "false");
    input.setAttribute("autocomplete", "off");
    return input;
  };

  const dom = document.createElement("div");
  dom.className = "cm-vsc-search";

  const searchField = makeInput("cm-vsc-input cm-vsc-find-input", "尋找");
  searchField.setAttribute("main-field", "true"); // 讓 openSearchPanel 能在面板已開啟時重新聚焦。
  searchField.value = query.search;
  const replaceField = makeInput("cm-vsc-input", "取代");
  replaceField.value = query.replace;

  let caseSensitive = query.caseSensitive;
  let wholeWord = query.wholeWord;
  let regexp = query.regexp;

  const commit = () => {
    const next = new SearchQuery({ search: searchField.value, replace: replaceField.value, caseSensitive, wholeWord, regexp });
    if (!next.eq(query)) {
      query = next;
      view.dispatch({ effects: setSearchQuery.of(next) });
    }
  };

  const makeOption = (label: string, title: string, get: () => boolean, set: (value: boolean) => void) => {
    const button = makeButton("cm-vsc-option", label, title, () => {
      set(!get());
      syncOptions();
      commit();
    });
    return { button, refresh: () => { button.classList.toggle("active", get()); button.setAttribute("aria-pressed", String(get())); } };
  };
  const caseOption = makeOption("Aa", "區分大小寫 (Alt+C)", () => caseSensitive, (value) => { caseSensitive = value; });
  const wordOption = makeOption("ab", "全字拼寫比對 (Alt+W)", () => wholeWord, (value) => { wholeWord = value; });
  const regexOption = makeOption(".*", "使用規則運算式 (Alt+R)", () => regexp, (value) => { regexp = value; });
  const options = [caseOption, wordOption, regexOption];
  const syncOptions = () => options.forEach((option) => option.refresh());
  syncOptions();

  const optionsBox = document.createElement("div");
  optionsBox.className = "cm-vsc-options";
  options.forEach((option) => optionsBox.append(option.button));

  const findField = document.createElement("div");
  findField.className = "cm-vsc-field";
  findField.append(searchField, optionsBox);

  const count = document.createElement("span");
  count.className = "cm-vsc-count";
  count.setAttribute("aria-live", "polite");

  const findRow = document.createElement("div");
  findRow.className = "cm-vsc-row";
  findRow.append(
    findField,
    count,
    makeButton("cm-vsc-icon-btn", arrowUpIcon, "上一個符合項目 (Shift+Enter)", () => { commit(); findPrevious(view); }),
    makeButton("cm-vsc-icon-btn", arrowDownIcon, "下一個符合項目 (Enter)", () => { commit(); findNext(view); }),
    makeButton("cm-vsc-icon-btn", closeIcon, "關閉 (Esc)", () => closeSearchPanel(view))
  );

  const replaceFieldBox = document.createElement("div");
  replaceFieldBox.className = "cm-vsc-field";
  replaceFieldBox.append(replaceField);

  const replaceRow = document.createElement("div");
  replaceRow.className = "cm-vsc-row cm-vsc-replace-row";
  replaceRow.append(
    replaceFieldBox,
    makeButton("cm-vsc-text-btn", "取代", "取代 (Enter)", () => { commit(); replaceNext(view); }),
    makeButton("cm-vsc-text-btn", "全部取代", "全部取代 (Ctrl+Alt+Enter)", () => { commit(); replaceAll(view); })
  );

  const rows = document.createElement("div");
  rows.className = "cm-vsc-rows";
  rows.append(findRow, replaceRow);

  const toggleReplace = makeButton("cm-vsc-toggle-replace", chevronIcon, "切換取代", () => {
    searchReplaceOpen = !searchReplaceOpen;
    dom.classList.toggle("replace-open", searchReplaceOpen);
  });
  dom.classList.toggle("replace-open", searchReplaceOpen);
  dom.append(toggleReplace, rows);

  const updateCount = () => {
    const current = getSearchQuery(view.state);
    findField.classList.remove("no-match");
    if (!current.search) { count.textContent = ""; return; }
    if (!current.valid) { count.textContent = "無結果"; findField.classList.add("no-match"); return; }
    const selection = view.state.selection.main;
    const limit = 9999;
    let total = 0;
    let index = 0;
    const cursor = current.getCursor(view.state);
    for (let result = cursor.next(); !result.done; result = cursor.next()) {
      total++;
      if (result.value.from === selection.from && result.value.to === selection.to) index = total;
      if (total >= limit) break;
    }
    if (total === 0) { count.textContent = "無結果"; findField.classList.add("no-match"); return; }
    count.textContent = `${index || "?"} / ${total >= limit ? `${limit}+` : total}`;
  };

  searchField.addEventListener("input", commit);
  replaceField.addEventListener("input", commit);
  searchField.addEventListener("change", commit);
  replaceField.addEventListener("change", commit);

  dom.addEventListener("keydown", (event) => {
    if (runScopeHandlers(view, event, "search-panel")) { event.preventDefault(); return; }
    if (event.altKey && !event.ctrlKey && !event.metaKey) {
      const option = event.code === "KeyC" ? caseOption : event.code === "KeyW" ? wordOption : event.code === "KeyR" ? regexOption : undefined;
      if (option) { event.preventDefault(); option.button.click(); return; }
    }
    if (event.key !== "Enter" || event.isComposing) return;
    if (event.target === searchField) {
      event.preventDefault();
      commit();
      (event.shiftKey ? findPrevious : findNext)(view);
    } else if (event.target === replaceField) {
      event.preventDefault();
      commit();
      if (event.ctrlKey && event.altKey) replaceAll(view);
      else replaceNext(view);
    }
  });

  updateCount();

  return {
    dom,
    top: true,
    mount() { searchField.focus(); searchField.select(); },
    update(update: ViewUpdate) {
      let queryChanged = false;
      for (const transaction of update.transactions) {
        for (const effect of transaction.effects) {
          if (effect.is(setSearchQuery) && !effect.value.eq(query)) {
            query = effect.value;
            queryChanged = true;
            if (searchField.value !== query.search) searchField.value = query.search;
            if (replaceField.value !== query.replace) replaceField.value = query.replace;
            caseSensitive = query.caseSensitive;
            wholeWord = query.wholeWord;
            regexp = query.regexp;
            syncOptions();
          }
        }
      }
      if (queryChanged || update.docChanged || update.selectionSet) updateCount();
    }
  };
}
