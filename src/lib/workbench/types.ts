export type ResultStatus = "AC" | "WA" | "RE" | "TLE";
export type ConsoleTab = "output" | "build" | "diff";
export type ResizeKind = "sidebar" | "main" | "console";
export interface TestCase {
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
export interface CompileResult { success: boolean; output: string; executablePath: string | null }
export interface RunResult { stdout: string; stderr: string; executionTimeMs: number; exitCode: number | null; timedOut: boolean; cancelled: boolean }
export interface CompareResult { accepted: boolean; firstDifference: number | null }
export interface LspSessionInfo {
  sessionId: string;
  eventName: string;
  rootUri: string;
  documentUri: string;
  clangdPath: string;
}
export interface ToolchainInfo { gxxPath: string; clangdPath: string; extracted: boolean }

export interface EditorTab { path: string; dirty: boolean }
export interface DiffSegment { text: string; bad?: boolean; missing?: string }
export interface DiffRow { kind: "same" | "changed" | "added" | "missing"; segments: DiffSegment[]; line: number | null }
export interface DiffView { rows: DiffRow[]; hasDiff: boolean; diffCount: number; firstNote: string; lineCount: number }
export interface DiffOp { op: "eq" | "del" | "ins"; a: number; b: number }
