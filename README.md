# IDE for Exam

面向高中競程練習的 Windows 桌面 C++ 工作台，提供本機編輯、G++ 編譯、執行與測資判題。

## MVP 功能

- 建立/開啟專案與 C++ 原始碼，透過 CodeMirror 編輯、搜尋及語法上色。
- 使用 clangd 提供 C++ completion、hover、診斷與定義跳轉。
- 呼叫本機 G++ 以 C++17 編譯，呈現編譯器輸出。
- 執行程式並檢視 stdin、stdout、stderr、結束碼與執行時間。
- 建立多組本機測資，批次執行並精確比較輸出；提供 AC、WA、RE、TLE 和差異位置。
- 對逾時程式終止執行程序。使用者程式以目前 Windows 使用者權限執行，並非安全沙箱。

功能階段、驗收標準及尚待發布前完成的工作，見 [MVP-ROADMAP.md](MVP-ROADMAP.md)。

## 開發環境

- Windows 10/11
- [Bun](https://bun.sh/) 1.3+
- Rust stable 與 Cargo
- GCC/G++（可放在工作區附近的 `mingw64/bin`，或加入 PATH）
- clangd（建議 22.1.6，放在工作區附近的 `clangd_22.1.6/bin`）

安裝 Bun 後，在專案根目錄執行：

```powershell
bun install --frozen-lockfile
bun run tauri dev
```

前端驗證：

```powershell
bun run check
bun run build
```

Rust 測試：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
``

g++ 和 clangd 已經內建在 IDE 中，無須依賴任何畚箕編譯器，以減少配置問題

## 技術組成

- Svelte 5、TypeScript、CodeMirror 6、codemirror-languageserver
- Tauri 2、Rust
- Bun 套件管理
- 本機 G++（C++17）

## 範圍

MVP 專注於本機解題工作流。ZeroJudge/TIOJ、登入/提交、雲端同步及 APCS 模擬考不在此版本範圍。
