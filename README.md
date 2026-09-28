# IDE for Exam

面向高中資訊競賽與 C++ 程式設計練習的 Windows 桌面工作台。

IDE for Exam 提供完整的本機解題流程，從建立專案、編輯 C++ 原始碼、程式碼補全，到編譯、執行與測資判題，都在桌面應用程式中完成。

> **目前版本定位：MVP**
>
> 專注於「本機 C++ 解題工作流」，不需要登入線上 OJ，也不依賴雲端服務。

---

## ✨ Features

### 📝 C++ 程式碼編輯

* 建立與開啟 C++ 專案
* 編輯、搜尋 C++ 原始碼
* CodeMirror 6 編輯器
* C++ 語法高亮
* 多檔案專案支援
* 支援 `.cpp`、`.cc`、`.cxx`、`.h`、`.hpp` 原始碼檔案

### 🧠 C++ Language Support

透過 **clangd** 提供語言伺服器功能：

* Code Completion
* Hover Information
* Diagnostics
* Definition Jump
* C++ Standard Library 符號解析
* 背景索引

IDE 會依照目前專案建立 `compile_commands.json`，讓 clangd 能夠使用與實際編譯相同的 C++17 編譯設定。

### 🔨 編譯

使用本機 **G++** 進行 C++17 編譯：

```text
g++ <source> -std=c++17 -O2 -o <executable>
```

編譯結果會提供：

* 編譯是否成功
* Compiler stdout
* Compiler stderr
* 產生的 executable 路徑

如果找不到 G++，IDE 會提供相應錯誤訊息。

### ▶️ 程式執行

支援直接執行目前已成功編譯的程式：

* stdin
* stdout
* stderr
* Exit Code
* Execution Time
* Timeout
* 手動取消執行

程式執行具有可設定的 timeout，上限為 300 秒；超時後會終止執行中的程序。

### 🧪 測資判題

可以建立多組本機測資並批次執行。

目前提供：

* Multiple Test Cases
* stdin 輸入
* stdout 輸出
* 精確輸出比較
* First Difference Position
* AC
* WA
* RE
* TLE

輸出比較會統一 Windows / Unix line ending，並忽略輸出最後多餘的換行，但會保留內容中間的空白行。

### 🛠️ Toolchain Management

IDE 可以自動尋找或初始化所需工具鏈：

* MinGW / G++
* clangd

工具鏈可以：

1. 使用環境變數指定
2. 使用工作區附近既有的工具鏈
3. 使用系統 `PATH`
4. 找不到時從 bundled ZIP resources 解壓安裝

目前 bundled toolchain 包含：

```text
resources/
├── mingw64.zip
└── clangd_22.1.6.zip
```

工具鏈會安裝至應用程式資料目錄中的 `toolchains`。

---

## ⚠️ Security Notice

**IDE for Exam 目前不是安全沙箱。**

使用者撰寫的 C++ 程式會以目前 Windows 使用者的權限直接執行。

因此，請只執行自己信任的程式碼。

目前的 timeout 機制主要用於避免無限迴圈或長時間執行的程式持續佔用資源，而不是安全隔離機制。

---

## 🖥️ System Requirements

### Runtime

* Windows 10 / 11
* x64

### Development

* Windows 10 / 11
* [Bun](https://bun.sh/) 1.3+
* Rust stable
* Cargo

G++ 與 clangd 可以由 IDE 的 bundled toolchain 管理，不需要使用者另外設定系統 PATH。

若需要使用自訂工具鏈，也可以透過環境變數指定：

```text
IDEFOREXAM_GXX
IDEFOREXAM_CLANGD
```

---

## 🚀 Development

Clone repository 後，在專案根目錄執行：

```powershell
bun install --frozen-lockfile
```

啟動 Tauri 開發環境：

```powershell
bun run tauri dev
```

---

## 🔍 Frontend Validation

執行 TypeScript / Svelte 檢查：

```powershell
bun run check
```

建立 production build：

```powershell
bun run build
```

---

## 🦀 Rust Tests

執行 Tauri backend 測試：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

目前 Rust backend 包含編譯、程式執行、輸出比較、執行取消，以及 clangd / toolchain 相關測試。

---

## 🏗️ Architecture

IDE for Exam 採用：

```text
┌─────────────────────────────────────┐
│              Svelte 5               │
│        TypeScript / CodeMirror      │
│                                     │
│  Editor │ Problems │ Console │ Test │
└──────────────────┬──────────────────┘
                   │
              Tauri Commands
                   │
┌──────────────────▼──────────────────┐
│              Rust Backend            │
│                                     │
│  Project Management                 │
│  C++ Compiler                       │
│  Program Runner                     │
│  Test Case Comparison               │
│  Toolchain Manager                  │
│  clangd / LSP Manager               │
└───────────┬─────────────────┬───────┘
            │                 │
         G++ / GCC          clangd
            │                 │
            └────────┬────────┘
                     │
                C++ Program
```

### Frontend

* Svelte 5
* TypeScript
* CodeMirror 6
* `codemirror-languageserver`

### Desktop Runtime

* Tauri 2

### Backend

* Rust
* Cargo

### Compiler

* G++ / GCC
* C++17
* `-O2`

### Language Server

* clangd
* Language Server Protocol (LSP)

---

## 📁 Project Structure

目前專案主要由以下部分組成：

```text
.
├── src/
│   └── ...
│
├── src-tauri/
│   ├── src/
│   │   ├── engine.rs
│   │   ├── lsp.rs
│   │   ├── toolchains.rs
│   │   ├── lib.rs
│   │   └── main.rs
│   │
│   └── Cargo.toml
│
├── resources/
│   ├── mingw64.zip
│   └── clangd_22.1.6.zip
│
├── README.md
└── ...
```

Rust backend 的主要職責如下：

| Module          | Responsibility                                  |
| --------------- | ----------------------------------------------- |
| `engine.rs`     | C++ 編譯、程式執行、timeout、取消、輸出比較                     |
| `lsp.rs`        | clangd 啟動、LSP communication、compile database    |
| `toolchains.rs` | G++ / clangd 搜尋、初始化與 bundled toolchain 安裝       |
| `lib.rs`        | Tauri commands 與 frontend / backend integration |
| `main.rs`       | Tauri application entry point                   |

---

## 🔄 Local Problem-Solving Workflow

典型使用流程：

```text
建立 / 開啟專案
       │
       ▼
編輯 C++ 程式
       │
       ▼
clangd 提供 Completion / Diagnostics
       │
       ▼
      編譯
       │
       ├── Compile Error ──► 修正程式
       │
       ▼
     執行
       │
       ├── RE
       ├── TLE
       └── Success
       │
       ▼
   執行測資
       │
       ▼
   比較 Expected Output
       │
       ├── AC
       └── WA + Difference
```

---

## 🎯 Scope

### MVP 包含

* 本機 C++ 專案管理
* C++ 程式碼編輯
* C++ Language Server
* G++ C++17 編譯
* 本機程式執行
* stdin / stdout / stderr
* Execution Timeout
* Program Cancellation
* 多組測資
* 批次測資執行
* 輸出比較
* AC / WA / RE / TLE 判定

### MVP 暫不包含

以下功能不在目前 MVP 範圍：

* ZeroJudge / TIOJ 線上提交
* 線上帳號與登入
* 線上題庫同步
* 雲端同步
* 線上評測服務
* APCS 模擬考系統

MVP 專注於本機解題工作流。

---

## 🗺️ Roadmap

功能階段、驗收標準，以及發布前仍需要完成的工作，請參考：

[`MVP-ROADMAP.md`](MVP-ROADMAP.md)

---

## 🧰 Technology Stack

* **Svelte 5**
* **TypeScript**
* **CodeMirror 6**
* **codemirror-languageserver**
* **Tauri 2**
* **Rust**
* **Bun**
* **G++ / GCC**
* **clangd**
* **C++17**

---

## 📌 Project Status

IDE for Exam 目前處於 **MVP 開發階段**。

目前核心的本機 C++ 解題流程已經建立，包括：

```text
Editor
  ↓
clangd
  ↓
Compile
  ↓
Run
  ↓
Test Cases
  ↓
Output Comparison
```

後續開發將以 MVP Roadmap 為準，逐步完善使用者體驗、測資管理與發布流程。
