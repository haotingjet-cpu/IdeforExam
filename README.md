# IDE for Exam

面向高中競程練習的 Windows 桌面 C++ 工作台，提供本機編輯、G++ 編譯、執行與測資判題。

## MVP 功能

- 建立/開啟專案與 C++ 原始碼，透過 CodeMirror 編輯、搜尋及語法上色。
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
```

IDE 會自動搜尋工作區或應用程式附近的 `mingw64/bin/g++.exe`，也會使用 PATH 中的 `g++`。若仍顯示找不到編譯器，可將含有 `g++.exe` 的目錄加入 PATH，或設定 `IDEFOREXAM_GXX` 指向完整執行檔路徑。

## 技術組成

- Svelte 5、TypeScript、CodeMirror 6
- Tauri 2、Rust
- Bun 套件管理
- 本機 G++（C++17）

## 範圍

MVP 專注於本機解題工作流。ZeroJudge/TIOJ、登入/提交、雲端同步及 APCS 模擬考不在此版本範圍。
