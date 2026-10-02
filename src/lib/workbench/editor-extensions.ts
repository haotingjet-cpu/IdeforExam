import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { indentUnit, syntaxTree } from "@codemirror/language";

// 輸入 "{" 時自動展開成 "{\n    |\n}"；字串、註解內，或游標後方還有內容時維持一般輸入。
export function autoExpandBrace(view: EditorView, from: number, to: number, text: string) {
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

export function editableExtensions(editable: boolean) {
  return [EditorView.editable.of(editable), EditorState.readOnly.of(!editable)];
}
