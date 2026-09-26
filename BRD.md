# 高中競程開源整合型 IDE

## Business Requirements Document（BRD）

**文件版本：** v1.0
**文件狀態：** Draft / Approved for MVP Development
**專案類型：** 資訊社開源專案
**主要使用者：** 高中資訊社社員、APCS 考生、高中競程學習者
**主要開發平台：** Windows
**開發模式：** Open Source / Community Driven

---

# 1. 文件目的

本文件定義「高中競程開源整合型 IDE」專案的業務需求、使用者需求、產品目標、功能範圍、成功指標與開發優先級。

本文件將作為後續：

* 軟體需求規格書（SRS）
* UI/UX 設計
* 技術架構設計
* 開發任務拆分
* 測試計畫
* 專案驗收

的上游需求依據。

本文件主要回答：

> **為什麼要做？**
>
> **為誰而做？**
>
> **要解決什麼問題？**
>
> **第一版必須做到什麼？**
>
> **什麼情況代表專案成功？**

---

# 2. 專案背景

高中競程學生在進行 C/C++ 程式解題時，通常需要同時使用 IDE、編譯器、瀏覽器與 Online Judge。

目前的工作流程可能涉及：

```text
開啟 IDE
   ↓
撰寫程式
   ↓
編譯
   ↓
開啟瀏覽器
   ↓
尋找題目
   ↓
複製測資
   ↓
回到 IDE
   ↓
執行
   ↓
人工比較輸出
   ↓
再次修改
```

原企劃指出，目前主要痛點包含開發環境設定複雜、IDE 與 OJ 網頁頻繁切換，以及缺乏方便的本地自動測資比對。

因此，本專案希望將競程練習中高頻率使用的功能整合至單一桌面應用程式。

---

# 3. Business Problem｜問題定義

## BP-01：競程開發環境建置門檻

初學者可能需要自行處理：

* 編譯器安裝
* PATH
* MinGW / GCC
* IDE 設定

環境問題可能在開始寫第一題之前就造成阻礙。

---

## BP-02：解題流程分散

題目、程式碼與提交平台分散於不同視窗。

使用者需要反覆切換：

> IDE ↔ Browser ↔ OJ

造成不必要的操作成本。

---

## BP-03：本地測試流程效率不足

使用者需要手動複製：

* Sample Input
* Sample Output

並自行執行與比較。

對初學者而言，這容易產生額外操作與比較錯誤。

---

# 4. Business Opportunity｜專案機會

本專案不是以商業營利為主要目的，而是以：

> **解決高中競程學生實際需求＋建立資訊社長期開源資產**

為核心。

本專案具有以下機會：

### 4.1 降低初學者門檻

讓使用者可以更快完成：

> 安裝 → 編譯 → 執行 → 測試

---

### 4.2 整合競程工作流程

將競程相關工具集中於同一個環境。

---

### 4.3 建立資訊社開源專案

提供社員學習：

* Git
* GitHub
* Software Engineering
* Frontend
* Rust
* C++
* Networking
* Testing
* Code Review

的實際環境。

---

### 4.4 建立跨屆傳承資產

專案不應依賴單一社員。

透過開源、文件與模組化架構，使後續社員可以接手維護。

---

# 5. Project Vision｜專案願景

> **讓高中生可以更容易開始、練習並完成競程解題。**

長期希望建立：

> **台灣高中競程學生友善的開源解題開發環境。**

---

# 6. Product Positioning｜產品定位

## 核心定位

> **為台灣高中競程學生打造的開源競程開發環境。**

本產品不是以取代 VS Code 等通用 IDE 為目標。

產品核心價值為：

> **整合競程解題工作流程。**

---

# 7. Target Users｜目標使用者

## 7.1 Primary User

### 高中資訊社社員

特別是：

* C++ 初學者
* APCS 準備者
* 程式競賽參與者

---

## 7.2 Secondary User

### 高中競程學生

包含：

* 其他學校資訊社
* APCS 學習者
* 高中競程社群

---

## 7.3 Future User

### 教師與指導者

未來可能使用：

* 題目練習
* 模擬考
* 學習管理

但不屬於 MVP 主要使用者。

---

# 8. User Needs｜使用者需求

## UN-01：我希望安裝後可以直接寫 C++

使用者不應需要理解複雜的環境變數設定。

---

## UN-02：我希望快速編譯程式

使用者應能透過單一操作完成編譯。

---

## UN-03：我希望快速執行程式

使用者應能直接執行目前程式並查看輸出。

---

## UN-04：我希望快速測試測資

使用者應能建立、管理並執行 Test Case。

---

## UN-05：我希望快速知道輸出哪裡錯

系統應提供 Output Diff。

---

## UN-06：我希望未來可以直接查看 OJ 題目

使用者可以在 IDE 中取得 OJ 題目內容，而不必一直切換瀏覽器。

---

# 9. Business Requirements｜業務需求

| ID    | 業務需求               | 優先級 |
| ----- | ------------------ | --- |
| BR-01 | 提供低門檻 C++ 開發環境     | P0  |
| BR-02 | 提供一鍵編譯             | P0  |
| BR-03 | 提供一鍵執行             | P0  |
| BR-04 | 提供本地 Test Case     | P0  |
| BR-05 | 提供 Output Diff     | P0  |
| BR-06 | 提供 Process Timeout | P0  |
| BR-07 | 整合 ZeroJudge 題目    | P1  |
| BR-08 | 建立可擴充 OJ Adapter   | P1  |
| BR-09 | 整合 TIOJ            | P2  |
| BR-10 | 提供 OJ Submit       | P2  |
| BR-11 | 提供 APCS Simulation | P3  |

---

# 10. MVP Scope｜MVP 範圍

## 10.1 In Scope

第一版必須包含：

### Editor

* C++ 程式碼編輯
* Syntax Highlight
* 基本程式碼操作

### Compiler

* GCC / G++
* Compile
* Compile Error 顯示

### Runner

* Run
* stdin
* stdout
* stderr
* execution time

### Local Judge

* Test Case
* Input
* Expected Output
* Actual Output
* Output Diff
* Timeout

---

# 11. Out of Scope｜MVP 不包含

以下功能第一版不實作：

* TIOJ
* OJ 自動登入
* OJ 自動提交
* APCS 模擬考
* 雲端同步
* 使用者帳號
* 雲端題庫
* AI Assistant
* 完整跨平台支援

原因：

> 避免初期 Scope 過大，使核心產品無法完成。

---

# 12. Functional Requirements｜功能需求

## FR-01 Editor

系統必須提供 C/C++ 程式碼編輯環境。

### Acceptance Criteria

使用者可以：

1. 建立 C++ 程式
2. 開啟程式
3. 修改程式
4. 儲存程式
5. 重新開啟程式

---

# 13. FR-02 Compiler

系統必須可以呼叫本機 C++ Compiler。

### Acceptance Criteria

使用者按下 Compile 後：

### 成功

顯示：

> Build Successful

### 失敗

顯示：

* Error
* Warning
* Compiler Output

---

# 14. FR-03 Program Runner

系統必須可以執行編譯完成的程式。

必須支援：

* Standard Input
* Standard Output
* Standard Error

---

# 15. FR-04 Test Case Manager

使用者可以建立：

```text
Test Case
├── Input
└── Expected Output
```

並執行測試。

---

# 16. FR-05 Output Diff

系統自動比較：

```text
Expected Output
        VS
Actual Output
```

並顯示差異。

---

# 17. FR-06 Timeout

系統必須能限制程式執行時間。

如果程式超過設定時間：

```text
Program
   ↓
Timeout
   ↓
Process Terminated
```

並向使用者顯示：

> Time Limit Exceeded

---

# 18. FR-07 ZeroJudge Integration

第二階段提供：

使用者輸入：

```text
題目代號
```

系統取得：

* 題目名稱
* 題目敘述
* Input
* Output
* Sample

原企劃已將 ZeroJudge 與 TIOJ 題目抓取列為核心需求之一。

---

# 19. FR-08 OJ Adapter

OJ 整合必須採用模組化設計：

```text
OJ Interface
      │
 ┌────┴────┐
 ↓         ↓
ZeroJudge  TIOJ
Adapter    Adapter
```

目的：

* 降低 OJ 改版影響
* 增加其他 OJ 的可能性
* 避免核心程式碼與單一平台高度耦合

---

# 20. FR-09 OJ Submission

列為後期需求。

未來可能支援：

* Login
* Credential Storage
* Submit Code
* Submission Status

帳號資訊不得以明文儲存。

---

# 21. Non-Functional Requirements｜非功能需求

## NFR-01 易用性

新使用者應能：

> 安裝後 5 分鐘內完成第一次 C++ 編譯與執行。

---

## NFR-02 效能

一般操作應保持流暢：

* Editor
* Test Case
* Output
* Diff

不得因一般測試操作造成明顯 UI 卡頓。

---

## NFR-03 穩定性

程式錯誤不應導致整個 IDE 崩潰。

特別是：

* Compiler Error
* Runtime Error
* Timeout
* Invalid Input

---

## NFR-04 安全性

若未來儲存 OJ 帳號：

* 不得明文儲存密碼
* 優先使用 OS Credential Store
* 不在 Log 中輸出敏感資訊

---

## NFR-05 可維護性

核心模組必須具有清楚邊界。

至少分為：

```text
Frontend
Core
Compiler
Runner
Judge
OJ Adapter
```

---

## NFR-06 可交接性

新社員應能依照文件完成：

> Clone → Setup → Build → Run

---

# 22. Technical Architecture｜技術架構

```text
┌──────────────────────────────────────┐
│              Frontend                │
│                                      │
│       Svelte + TypeScript            │
│                                      │
│ Editor │ Problem │ Test │ Diff       │
└─────────────────┬────────────────────┘
                  │
             Tauri IPC
                  │
                  ↓
┌──────────────────────────────────────┐
│              Rust Core               │
│                                      │
│ Compiler │ Runner │ Judge │ OJ       │
└─────────────┬───────────┬────────────┘
              │           │
              ↓           ↓
          Local OS      Internet
              │           │
              ↓           ↓
             GCC       OJ Adapter
```

原始企劃採用 Tauri、Rust、Svelte 與 TypeScript 的方向，並以 Rust 處理本機編譯與 Process Control。

---

# 23. Priority｜需求優先級

採用 MoSCoW：

## Must Have

MVP 必須完成：

* Editor
* Compile
* Run
* Test Case
* Diff
* Timeout

---

## Should Have

MVP 後優先：

* ZeroJudge Integration
* Sample Test 自動匯入
* OJ Adapter

---

## Could Have

後期：

* TIOJ
* OJ Submit
* APCS Simulation

---

## Won't Have

目前不開發：

* 雲端 IDE
* AI Coding Assistant
* 完整線上課程平台
* 社交系統

---

# 24. User Journey｜使用者流程

## 新使用者

```text
下載
 ↓
安裝
 ↓
開啟 IDE
 ↓
建立 C++ 程式
 ↓
輸入程式碼
 ↓
Compile
 ↓
Run
 ↓
加入 Test Case
 ↓
執行 Test
 ↓
查看 Diff
```

成功條件：

> 使用者完成第一題程式。

---

# 25. OJ User Journey｜第二階段

```text
輸入題號
 ↓
取得題目
 ↓
閱讀題目
 ↓
撰寫程式
 ↓
本地測試
 ↓
修正
 ↓
提交
```

目標：

> 降低競程解題過程中的工具切換。

---

# 26. Success Metrics｜成功指標

本專案不以營收為主要成功標準。

## 使用者指標

第一階段：

* 至少 20 名實際測試使用者
* 至少 100 次解題流程
* 至少 70% 測試使用者願意再次使用

---

## 使用體驗指標

新使用者：

> 5 分鐘內完成第一次編譯與執行。

---

## 技術指標

MVP：

* Core workflow 完成
* CI Build
* Automated Test
* Release Package

---

## 開源指標

正式公開版本必須具有：

* README
* Installation Guide
* CONTRIBUTING.md
* Issue Template
* Pull Request Template
* Architecture Documentation

---

# 27. Project Governance｜專案治理

專案採資訊社共同維護模式。

## Maintainer

負責：

* Architecture
* Release
* Code Review
* 技術方向

## Contributors

負責：

* Feature
* Bug Fix
* Documentation
* Testing

## 新舊屆交接

每一學年必須完成：

* Repository 交接
* 技術文件更新
* Maintainer 培訓
* Development Environment 教學

---

# 28. Risk Management｜風險管理

| Risk     | Impact | Probability | Mitigation |
| -------- | ------ | ----------- | ---------- |
| Scope 過大 | 高      | 高           | MVP        |
| OJ 改版    | 高      | 高           | Adapter    |
| 核心社員畢業   | 高      | 高           | 文件化        |
| GCC 設定問題 | 中      | 中           | 自動化        |
| OJ 登入安全  | 高      | 中           | 延後開發       |
| 使用者不足    | 中      | 中           | 校內先行驗證     |
| 維護成本過高   | 中      | 中           | 開源協作       |

---

# 29. Development Roadmap｜開發路線

## Phase 0

**Research**

2 週

成果：

> Technical Prototype

---

## Phase 1

**MVP**

6–8 週

成果：

> Editor + Compiler + Runner + Local Judge

---

## Phase 2

**Internal Beta**

2–4 週

成果：

> 資訊社社員實際測試

---

## Phase 3

**OJ Integration**

4–6 週

成果：

> ZeroJudge Adapter

---

## Phase 4

**Public Release**

成果：

> Open Source v1.0

---

# 30. Acceptance Criteria｜專案驗收

MVP 必須完成以下完整流程：

```text
建立 C++ 程式
      ↓
編輯
      ↓
Compile
      ↓
Run
      ↓
輸入 Test Case
      ↓
執行
      ↓
取得 Output
      ↓
Diff
      ↓
判斷 AC / WA
```

並且：

1. 新使用者能完成安裝。
2. 能正常編譯 C++。
3. 能執行程式。
4. 能建立 Test Case。
5. 能比較 Output。
6. 能處理 Runtime Error。
7. 能處理 Timeout。
8. 核心流程不需開啟 OJ 即可完成。

---

# 31. Project Success Definition｜成功定義

本專案成功不代表：

> 「我們寫了很多程式碼。」

而代表：

> **資訊社社員真的使用它解題。**

最低成功標準：

```text
20+ Users
100+ Solving Sessions
70%+ Repeat Usage
5 Minutes → First Successful Compile
```

並且專案具備：

> **跨屆維護能力。**

---

# 32. Final Business Requirement Statement

本專案最核心的需求可以濃縮為：

> **高中競程學生需要一個低門檻、整合化且開源的程式開發環境，使其能夠在不處理複雜環境設定及頻繁工具切換的情況下，完成程式編寫、編譯、本地測試與輸出驗證。**

因此：

> **MVP 的核心價值不是「功能很多」，而是「讓解題流程變簡單」。**

---

# 33. Decision

**Project Status：APPROVED**

**Approval Scope：MVP**

第一階段批准：

> **Editor → Compile → Run → Test Case → Diff → Timeout**

後續功能：

> **ZeroJudge → OJ Adapter → TIOJ → Submission → APCS Simulation**

必須依照實際使用者需求、開發能力與維護成本逐階段審查。

**最終原則：**

> **先解決問題，再增加功能。**

> **先服務資訊社，再服務更大的高中競程社群。**

> **先建立可維護的開源專案，再追求完整功能。**
