import type { DiffOp, DiffRow, DiffSegment, DiffView } from "./types";

export function splitDiffLines(text: string) {
  if (text === "") return [] as string[];
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}
export function numberedLines(text: string) { return splitDiffLines(text); }
export function gutterStyle(count: number) { return "--gw:" + Math.max(String(count).length, 2) + "ch"; }
// 通用 LCS diff；輸入太大時退回逐位置比較，避免卡住 UI
export function diffOps<T>(a: T[], b: T[]): DiffOp[] {
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
export function diffChars(expected: string, actual: string): DiffSegment[] {
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
export function buildDiff(expected: string, actual: string): DiffView {
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
