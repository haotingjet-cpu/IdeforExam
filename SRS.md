# 高中競程開源整合型 IDE

## Software Requirements Specification（SRS）

**文件版本：** v1.0
**文件狀態：** Approved for MVP Development
**文件類型：** Software Requirements Specification
**專案類型：** 資訊社開源專案
**主要平台：** Windows
**主要語言：** C++
**Frontend：** Svelte + TypeScript
**Desktop Framework：** Tauri
**Backend：** Rust
**Compiler：** GCC / G++

---

# 1. 文件目的

本文件定義「高中競程開源整合型 IDE」的軟體需求與系統行為。

本文件承接 BRD 所定義的業務需求，將其轉換為：

* 功能需求
* 非功能需求
* 使用者流程
* 系統介面
* 資料結構
* 錯誤處理
* 安全需求
* 測試與驗收條件
* MVP 與後續版本範圍

本文件的主要使用者為：

* 開發者
* UI/UX 設計者
* 測試人員
* 專案 Maintainer
* 資訊社社員

---

# 2. System Overview｜系統概述

本系統是一套桌面型競程開發環境，目標是將高中競程學生常用的程式開發與本地測試流程整合至單一應用程式。

核心工作流程：

```text
┌──────────────┐
│   建立程式    │
└──────┬───────┘
       ↓
┌──────────────┐
│   編輯 C++   │
└──────┬───────┘
       ↓
┌──────────────┐
│    Compile   │
└──────┬───────┘
       ↓
┌──────────────┐
│     Run      │
└──────┬───────┘
       ↓
┌──────────────┐
│  Test Cases  │
└──────┬───────┘
       ↓
┌──────────────┐
│  Output Diff │
└──────────────┘
```

第二階段：

```text
OJ 題號
   ↓
取得題目
   ↓
閱讀題目
   ↓
本地測試
   ↓
提交
```

---

# 3. Scope｜系統範圍

## 3.1 MVP In Scope

MVP 必須包含：

1. C++ Editor
2. Project / File 管理
3. GCC / G++ Compile
4. Program Run
5. Standard Input
6. Standard Output
7. Standard Error
8. Test Case 管理
9. Output Diff
10. Process Timeout
11. 基本錯誤處理

---

## 3.2 Phase 2

MVP 完成後：

1. ZeroJudge 題目取得
2. 題目 Markdown / LaTeX Render
3. Sample Test 自動匯入
4. OJ Adapter Architecture

---

## 3.3 Phase 3

後續版本：

1. TIOJ
2. OJ Login
3. Secure Credential Storage
4. OJ Submission
5. Submission Status

---

## 3.4 Phase 4

長期功能：

1. APCS Simulation
2. 練習紀錄
3. 題目管理
4. 學習分析

---

# 4. Out of Scope｜非本版本範圍

MVP 不包含：

* 雲端 IDE
* 使用者帳號
* 雲端同步
* AI Code Assistant
* 線上課程
* 社群聊天
* 完整跨平台支援
* TIOJ
* OJ 自動登入
* OJ 自動提交
* APCS Simulation

---

# 5. User Roles｜使用者角色

## 5.1 Student

主要使用者。

可以：

* 建立程式
* 編輯程式
* 編譯
* 執行
* 建立測資
* 執行測試
* 查看 Diff
* 查看錯誤

---

## 5.2 Developer / Contributor

開發者負責：

* 修改程式碼
* 新增功能
* 修正 Bug
* 建立 Pull Request

此角色屬於開源專案治理，而非產品登入角色。

---

## 5.3 Maintainer

負責：

* Code Review
* Release
* Architecture
* Issue 管理
* 版本維護

---

# 6. System Architecture｜系統架構

```text
┌─────────────────────────────────────────┐
│                 UI Layer                │
│                                         │
│       Svelte + TypeScript               │
│                                         │
│ Editor │ Terminal │ Test │ Problem     │
└────────────────────┬────────────────────┘
                     │
                 Tauri IPC
                     │
                     ↓
┌─────────────────────────────────────────┐
│                Core Layer               │
│                                         │
│  Project │ Compiler │ Runner │ Judge   │
└─────────────┬────────────────┬──────────┘
              │                │
              ↓                ↓
       Local File System     GCC/G++
                              
              ↓
       ┌─────────────────┐
       │   OJ Adapter    │
       │    Phase 2+     │
       └────────┬────────┘
                ↓
             Internet
```

原始企劃的技術架構即採用 Svelte + TypeScript 前端、Tauri/Rust Bridge，以及 Rust Core 處理 Compiler、Crawler 與 Judge。

---

# 7. Functional Requirements｜功能需求

---

# FR-001 Project Management

## 7.1 Description

系統必須提供基本的專案與程式檔案管理。

## 7.2 Functional Requirements

使用者可以：

* 建立專案
* 開啟專案
* 建立 `.cpp` 檔案
* 開啟 `.cpp` 檔案
* 儲存檔案
* 關閉專案

## 7.3 Input

```text
Project Name
File Name
File Path
```

## 7.4 Output

系統建立指定專案與檔案。

## 7.5 Acceptance Criteria

* 可以建立新的 C++ 專案。
* 可以儲存 `.cpp`。
* 重新開啟 IDE 後可以再次開啟檔案。
* 不得因關閉 Editor 而遺失已儲存內容。

---

# FR-002 Code Editor

## 8.1 Description

系統必須提供 C/C++ 程式碼編輯功能。

## 8.2 Required Features

MVP：

* Syntax Highlight
* Line Number
* Text Editing
* Search
* Basic Auto Indentation

---

## 8.3 Future Features

後續可以加入：

* IntelliSense
* Code Completion
* Clang-format
* Symbol Navigation

---

# FR-003 Compiler

## 9.1 Description

系統必須能呼叫本機 C++ Compiler。

初期支援：

> GCC / G++

## 9.2 Compile Flow

```text
User
 ↓
Compile
 ↓
Check Source
 ↓
Invoke GCC
 ↓
Capture Output
 ↓
Display Result
```

## 9.3 Success

顯示：

```text
Build Successful
```

並產生可執行檔。

## 9.4 Failure

顯示：

```text
Build Failed
```

並顯示 Compiler Output。

## 9.5 Acceptance Criteria

### AC-003-01

給予合法 C++ 程式：

```cpp
#include <iostream>

int main() {
    std::cout << "Hello";
    return 0;
}
```

系統應成功編譯。

### AC-003-02

給予存在 Syntax Error 的程式：

系統應：

1. 編譯失敗
2. 不執行舊版本程式
3. 顯示錯誤訊息

---

# FR-004 Program Runner

## 10.1 Description

使用者可以執行成功編譯的程式。

## 10.2 Input

Program

Optional:

```text
stdin
```

## 10.3 Output

系統顯示：

* stdout
* stderr
* Execution Time
* Exit Code

---

# FR-005 Standard Input

系統應提供輸入區域：

```text
┌────────────────────┐
│ Standard Input     │
│                    │
│ 5                  │
│ 1 2 3 4 5          │
└────────────────────┘
```

使用者執行程式後，系統將內容傳入：

```text
stdin
```

---

# FR-006 Standard Output

系統必須將程式：

```text
stdout
```

顯示於 Output Panel。

---

# FR-007 Standard Error

程式產生的：

```text
stderr
```

必須與正常輸出有所區別。

例如：

```text
STDOUT
...

STDERR
...
```

---

# FR-008 Test Case Management

## 13.1 Description

使用者可以建立多組測試案例。

## 13.2 Data Model

```text
TestCase
├── id
├── name
├── input
└── expectedOutput
```

## 13.3 Example

```text
Test Case 01

Input:
5
1 2 3 4 5

Expected:
15
```

---

# FR-009 Execute Test Case

使用者可以對單一 Test Case 執行：

```text
Compile
 ↓
Run
 ↓
Inject Input
 ↓
Capture Output
 ↓
Compare
```

---

# FR-010 Batch Test

系統應允許一次執行多組 Test Case。

例如：

```text
Test 01  ✓
Test 02  ✓
Test 03  ✗
Test 04  ✓
```

---

# FR-011 Output Diff

## 15.1 Description

系統比較：

```text
Expected Output
```

與：

```text
Actual Output
```

## 15.2 Result

### 相同

```text
AC
```

### 不同

```text
WA
```

並顯示差異。

---

# FR-012 Output Comparison

MVP 預設採文字輸出比較。

系統至少應處理：

* Line difference
* Character difference
* Newline difference

未來可加入：

* Whitespace normalization
* Token-based comparison

但不列入 MVP 必要功能。

---

# FR-013 Runtime Error

如果程式執行時發生錯誤：

系統不得讓 IDE 整體崩潰。

顯示：

```text
Runtime Error
Exit Code: XXX
```

---

# FR-014 Timeout

## 17.1 Description

防止使用者程式無限執行。

例如：

```cpp
while (true) {}
```

## 17.2 Flow

```text
Run
 ↓
Timer Start
 ↓
Program Running
 ↓
Timeout
 ↓
Terminate Process
 ↓
Display TLE
```

## 17.3 Result

```text
Time Limit Exceeded
```

原企劃亦規劃由 Rust `std::process` 控制本機程式並處理 timeout。

---

# FR-015 Process Termination

系統必須能停止：

* 正常執行中的程式
* Timeout 程式
* 使用者手動停止的程式

不得留下持續執行的子程序。

---

# FR-016 Problem Viewer

Phase 2。

系統提供題目閱讀區域：

```text
┌───────────────────────────┐
│ Problem                   │
├───────────────────────────┤
│ Description               │
│                           │
│ Input                     │
│                           │
│ Output                    │
│                           │
│ Sample                    │
└───────────────────────────┘
```

---

# FR-017 ZeroJudge Adapter

Phase 2。

使用者輸入題號後：

```text
題號
 ↓
ZeroJudge Adapter
 ↓
HTTP Request
 ↓
Parser
 ↓
Problem Model
 ↓
UI
```

---

# FR-018 Problem Parser

系統需要將 OJ 頁面資料轉換為統一格式：

```text
Problem
├── id
├── title
├── description
├── input
├── output
├── samples
└── metadata
```

---

# FR-019 Sample Test Import

如果題目包含：

```text
Sample Input
Sample Output
```

系統可以建立：

```text
Test Case
```

以供本地測試。

---

# FR-020 OJ Adapter Interface

所有 OJ 必須透過統一 Interface。

概念：

```text
interface OJAdapter {

    getProblem(id)

    getSamples(id)

    login()

    submit(code)

}
```

實際 API 與實作語言依技術設計決定。

---

# FR-021 TIOJ Adapter

Phase 3。

支援：

* 題目取得
* Sample 取得
* 未來提交

---

# FR-022 Secure Credential Storage

Phase 3。

如果系統需要儲存 OJ 登入資訊：

不得：

* 寫入明文設定檔
* 寫入 Log
* 寫入 Git
* 寫入一般文字檔

應使用 OS 提供的 Credential Store。

---

# FR-023 OJ Submission

Phase 3。

使用者可以：

```text
Submit
 ↓
Select OJ
 ↓
Select Problem
 ↓
Submit Code
 ↓
Receive Status
```

---

# FR-024 Submission Status

系統未來可以顯示：

```text
Pending
Accepted
Wrong Answer
Runtime Error
Time Limit Exceeded
Compilation Error
```

---

# 8. Non-Functional Requirements｜非功能需求

# NFR-001 Usability

新使用者應能：

> 在 5 分鐘內完成第一次 C++ 程式編譯與執行。

---

# NFR-002 Performance

一般 UI 操作不得因正常 Test Case 操作而造成明顯卡頓。

Compile / Run / Judge 應於 Backend 執行，不應阻塞 UI Thread。

---

# NFR-003 Reliability

以下情況不得造成整個 IDE 崩潰：

* Compiler Error
* Runtime Error
* Timeout
* Invalid Test Case
* Invalid File
* OJ Request Failure

---

# NFR-004 Security

MVP：

* 不需要 OJ 帳號
* 不儲存使用者密碼

Phase 3：

* 使用 OS Credential Store
* 密碼不得進入 Log
* 密碼不得進入 Git Repository

---

# NFR-005 Maintainability

系統必須採模組化架構。

至少包含：

```text
frontend/
core/
compiler/
runner/
judge/
oj/
```

---

# NFR-006 Portability

MVP：

> Windows First

未來：

* Linux
* macOS

---

# NFR-007 Open Source

專案程式碼必須：

* 使用 Git 管理
* 提供 LICENSE
* 提供 README
* 提供 CONTRIBUTING
* 提供 Issue Template
* 提供 Release

---

# NFR-008 Documentation

開發文件至少包含：

1. Installation
2. Development Setup
3. Architecture
4. Contribution Guide
5. Release Guide

---

# 9. Data Requirements｜資料需求

## 9.1 Project

```text
Project
├── id
├── name
└── path
```

---

## 9.2 Source File

```text
SourceFile
├── path
├── language
└── content
```

---

## 9.3 Test Case

```text
TestCase
├── id
├── name
├── input
└── expectedOutput
```

---

## 9.4 Test Result

```text
TestResult
├── testCaseId
├── actualOutput
├── expectedOutput
├── status
├── executionTime
└── exitCode
```

Status：

```text
AC
WA
RE
TLE
CE
```

---

# 10. Error Handling｜錯誤處理

系統不得只顯示：

> Error

而應盡量提供使用者可以理解的資訊。

---

## EH-001 Compiler Not Found

```text
找不到 C++ Compiler。

請確認 GCC / G++ 是否已安裝，
或依照安裝指南設定 Compiler。
```

---

## EH-002 Compile Error

顯示 GCC 原始錯誤訊息。

---

## EH-003 Runtime Error

顯示：

```text
Program terminated unexpectedly.

Exit Code: XXX
```

---

## EH-004 Timeout

顯示：

```text
Time Limit Exceeded
```

---

## EH-005 File Error

顯示：

```text
Unable to open file.
```

並提供：

> Retry / Open Folder

---

## EH-006 OJ Connection Error

Phase 2：

```text
Unable to connect to Online Judge.

Please check your network connection
or try again later.
```

不得將網路錯誤誤判為題目不存在。

---

# 11. UI Requirements｜介面需求

MVP 建議採以下 Layout：

```text
┌──────────────────────────────────────────────────┐
│ Menu / Toolbar                                   │
├──────────────┬───────────────────────┬───────────┤
│              │                       │           │
│ File / Test  │       Editor          │ Problem   │
│              │                       │           │
│              │                       │           │
├──────────────┴───────────────────────┴───────────┤
│ Input │ Output │ Error │ Test Result             │
└──────────────────────────────────────────────────┘
```

---

# 12. User Stories｜使用者故事

## US-001

> 身為 C++ 初學者，我希望可以直接建立 C++ 程式，以便開始寫題。

### Acceptance Criteria

* 可以建立 `.cpp`
* 可以編輯
* 可以儲存

---

## US-002

> 身為競程學生，我希望按一個按鈕就能編譯程式，以便快速確認程式是否可以建立。

---

## US-003

> 身為競程學生，我希望直接輸入測資執行程式，以便測試演算法。

---

## US-004

> 身為競程學生，我希望建立多組 Test Case，以便測試不同情況。

---

## US-005

> 身為競程學生，我希望系統自動比較輸出，以便快速知道答案是否正確。

---

## US-006

> 身為競程學生，我希望無限迴圈會被自動停止，以避免程式一直佔用電腦資源。

---

## US-007

> 身為競程學生，我希望可以直接查看 OJ 題目，以便減少瀏覽器與 IDE 之間的切換。

---

# 13. Security Requirements｜安全需求

MVP 不處理 OJ 帳號，因此安全需求主要集中於本機程式執行。

## SR-001

使用者程式必須以獨立 Process 執行。

## SR-002

Compiler 與 User Program 不得阻塞 UI。

## SR-003

系統必須能終止失控 Process。

## SR-004

系統不得將未經使用者同意的程式碼上傳至遠端。

## SR-005

Phase 3 儲存 OJ Credential 時，必須使用 OS Credential Store。

---

# 14. Privacy Requirements｜隱私需求

MVP：

> 不需要建立使用者帳號。

因此：

* 不收集姓名
* 不收集 Email
* 不建立使用者資料庫
* 不上傳程式碼

除非未來明確增加雲端服務。

---

# 15. Logging Requirements｜紀錄需求

系統可以記錄：

* Compile Result
* Runtime Result
* Error
* Application Error

不得記錄：

* OJ Password
* Credential
* Token
* Cookie

---

# 16. Testing Requirements｜測試需求

## 16.1 Unit Test

至少測試：

* Output Comparator
* Test Case Parser
* Process Result Parser
* OJ Parser

---

## 16.2 Integration Test

測試：

```text
Editor
 ↓
Compiler
 ↓
Runner
 ↓
Judge
```

---

## 16.3 End-to-End Test

完整測試：

```text
建立專案
 ↓
寫程式
 ↓
Compile
 ↓
Run
 ↓
Test
 ↓
Diff
```

---

# 17. MVP Acceptance Test｜MVP 驗收測試

## AT-001 Hello World

輸入：

```cpp
#include <iostream>

int main() {
    std::cout << "Hello World";
}
```

預期：

```text
Build Successful
```

---

## AT-002 Compile Error

輸入有 Syntax Error 的 C++。

預期：

```text
Build Failed
```

並顯示錯誤位置。

---

## AT-003 Standard Input

輸入：

```text
5
```

程式讀取後輸出：

```text
25
```

預期：

> Output 正確。

---

## AT-004 Wrong Answer

Expected：

```text
10
```

Actual：

```text
11
```

預期：

> WA

並顯示差異。

---

## AT-005 Multiple Test Cases

建立：

```text
Test 01
Test 02
Test 03
```

系統可以依序執行。

---

## AT-006 Timeout

執行無限迴圈：

```cpp
while (true) {}
```

預期：

```text
TLE
```

且 Process 被終止。

---

# 18. Release Requirements｜發布需求

MVP Release 必須包含：

* Windows Installer
* Version Number
* Release Notes
* README
* Installation Guide

版本格式：

```text
v0.1.0
```

---

# 19. Definition of Done｜完成定義

一項功能只有在以下條件全部完成時才算 Done：

1. Code 完成
2. Code Review 完成
3. Unit Test 完成
4. Integration Test 完成
5. Documentation 更新
6. UI 完成
7. Error Handling 完成
8. Issue 關閉

---

# 20. MVP Definition of Done

MVP 必須：

### 功能

* [ ] Project Management
* [ ] C++ Editor
* [ ] GCC Compile
* [ ] Program Run
* [ ] stdin
* [ ] stdout
* [ ] stderr
* [ ] Test Case
* [ ] Batch Test
* [ ] Output Diff
* [ ] Timeout
* [ ] Runtime Error Handling

### 工程

* [ ] Git Repository
* [ ] CI
* [ ] Unit Tests
* [ ] Documentation
* [ ] Installer
* [ ] Release

### 使用者驗證

* [ ] 20 名以上測試使用者
* [ ] 100 次以上實際解題
* [ ] 收集使用者回饋
* [ ] 修正重大 Bug

---

# 21. Future Requirements｜未來需求

以下不屬於 MVP，但保留架構擴充空間：

## OJ

* ZeroJudge
* TIOJ
* 其他 Online Judge

## Competition

* APCS Simulation
* Contest Mode
* Timer

## Learning

* Problem History
* Statistics
* Wrong Answer Analysis

## Platform

* Linux
* macOS

---

# 22. Requirement Traceability Matrix｜需求追蹤矩陣

| BRD               | SRS                    | MVP       |
| ----------------- | ---------------------- | --------- |
| BR-01 低門檻開發       | FR-001, FR-002, FR-003 | ✅         |
| BR-02 一鍵編譯        | FR-003                 | ✅         |
| BR-03 一鍵執行        | FR-004                 | ✅         |
| BR-04 Test Case   | FR-008, FR-009, FR-010 | ✅         |
| BR-05 Output Diff | FR-011, FR-012         | ✅         |
| BR-06 Timeout     | FR-014, FR-015         | ✅         |
| BR-07 ZeroJudge   | FR-016, FR-017, FR-019 | ❌ Phase 2 |
| BR-08 OJ Adapter  | FR-020                 | ❌ Phase 2 |
| BR-09 TIOJ        | FR-021                 | ❌ Phase 3 |
| BR-10 OJ Submit   | FR-023                 | ❌ Phase 3 |
| BR-11 APCS        | Future Requirements    | ❌ Phase 4 |

---

# 23. Project Constraints｜專案限制

本專案受到以下限制：

### 人力

由資訊社社員利用課餘時間開發。

### 技術能力

開發者可能具有不同程度的：

* C++
* Rust
* TypeScript
* Git

經驗。

### 時程

不得因追求完整功能而影響社團正常活動。

### 維護

專案必須考慮社員畢業後的交接。

---

# 24. Design Principles｜設計原則

## Principle 1

> **Simple First**

先解決核心問題。

---

## Principle 2

> **Local First**

MVP 核心功能不依賴雲端。

---

## Principle 3

> **Modular**

OJ 等外部服務必須模組化。

---

## Principle 4

> **Open Source First**

文件、Issue、Code Review 與開發流程從一開始就按照開源專案設計。

---

## Principle 5

> **User Driven**

功能是否加入，以實際使用者需求為依據，而不是單純因為技術上有趣。

---

# 25. Final Specification

本專案 MVP 的核心 Software Requirement 可濃縮為：

> **系統必須讓一名高中競程學生，在 Windows 電腦上開啟 IDE 後，可以建立 C++ 程式、編譯、執行、輸入測資、管理多組 Test Case、自動比較輸出，並能安全處理 Compile Error、Runtime Error 與 Timeout。**

完成上述流程後，系統才進一步整合 Online Judge。

---

# 26. Final Acceptance Statement

當以下流程可以穩定完成：

```text
┌───────────────┐
│ Create Project│
└───────┬───────┘
        ↓
┌───────────────┐
│   Edit C++    │
└───────┬───────┘
        ↓
┌───────────────┐
│    Compile    │
└───────┬───────┘
        ↓
┌───────────────┐
│      Run      │
└───────┬───────┘
        ↓
┌───────────────┐
│   Test Case   │
└───────┬───────┘
        ↓
┌───────────────┐
│   Judge/Diff  │
└───────┬───────┘
        ↓
     AC / WA
```

且通過：

* 功能測試
* 整合測試
* 使用者測試
* Stability Test
* Documentation Review

則可宣布：

> **MVP Release Candidate 完成。**

---

# 27. 文件狀態

**Status：APPROVED FOR DEVELOPMENT**

**Current Target：v0.1 MVP**

**下一階段文件：**

> Technical Design Document（TDD）
> UI/UX Specification
> Test Plan
> Development Backlog

**核心開發原則：**

> **先讓第一題跑起來，再讓第一百題跑得更舒服。**
