export function clamp(value: number, min: number, max: number) { return Math.round(Math.max(min, Math.min(max, value))); }
export function joinPath(folder: string, filename: string) { return `${folder}${folder.includes("\\") ? "\\" : "/"}${filename}`; }
