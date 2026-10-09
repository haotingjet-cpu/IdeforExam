import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";

// 輸入 "{" 時自動展開成 "{}"；字串、註解內，或游標後方還有內容時維持一般輸入。
export function autoExpandBrace(view: EditorView, from: number, to: number, text: string) {
  if (text !== "{" || from !== to) return false;
  const state = view.state;
  if (/String|Comment|Char/i.test(syntaxTree(state).resolveInner(from, -1).name)) return false;
  const closingAlreadyPresent = state.doc.sliceString(from, from + 1) === ")";
  view.dispatch({
    changes: { from, to, insert: closingAlreadyPresent ? "{" : "{}" },
    selection: { anchor: from + 1 },
    userEvent: "input.type"
  });
  return true;
}

export function autoCloseParenthesis(view: EditorView, from: number, to: number, text: string) {
  if (text !== "(" || from !== to) return false;
  const state = view.state;
  if (/String|Comment|Char/i.test(syntaxTree(state).resolveInner(from, -1).name)) return false;
  const closingAlreadyPresent = state.doc.sliceString(from, from + 1) === ")";
  view.dispatch({
    changes: { from, to, insert: closingAlreadyPresent ? "(" : "()" },
    selection: { anchor: from + 1 },
    userEvent: "input.type"
  });
  return true;
}

export function editableExtensions(editable: boolean) {
  return [EditorView.editable.of(editable), EditorState.readOnly.of(!editable)];
}
