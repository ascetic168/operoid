# GBrainStudio Employee 架構設計

## 核心理念

-   Employee 是第一級物件（First-class Object）。
-   Brain 是 Employee 的一個屬性，而不是系統主體。
-   Brain 負責「知道什麼」；Employee 負責「做什麼」。

## Workspace

``` text
Workspace
├── Employees
├── Brains
├── Projects
├── Artifacts
├── Knowledge
├── Tasks
├── Tools
└── Models
```

## Employee

``` text
Employee
├── Identity
├── Brain
├── Role
├── Capability
├── Resources
├── State
├── Inbox
├── Commitments
├── Memory
└── Metrics
```

### 1. Identity

-   ID
-   Name
-   Avatar
-   Department
-   Title
-   Description
-   Owner

### 2. Brain

Employee 僅引用 Brain。 Brain 包含： - Version - Knowledge - Prompt -
Long-term Memory - Embedding - Model Preference

Brain 可由多位 Employee 共用。

### 3. Role

定義工作責任，而非知識。

包含： - Mission - Responsibilities - Authority - KPI - SOP - Policies

例如 AI 採購助理： - 建立 PO - 追蹤交期 - 催貨 - 更新 ERP - 通知相關單位

### 4. Capability

定義「能做什麼」。

例如： - Email - ERP - Browser - SQL - Python - CAD - MCP

Capability 是能力，不是工具。

### 5. Resources

Employee 可使用的實際資源： - ERP - Exchange - Claude Code - Python -
AutoCAD - MariaDB - PostgreSQL

### 6. State

Runtime 狀態： - Idle - Working - Waiting - Sleeping - Paused - Error

### 7. Inbox

工作佇列。

Agent 每次被喚醒時，只需處理 Inbox。

### 8. Commitments

長期責任，例如： - 持續追蹤 PO 至收貨 - 合約追蹤 - ISO 稽核

Commitment 可持續數週甚至數月。

### 9. Memory

工作記憶，而非知識。

例如： - 今天已聯絡供應商 - 對方承諾週五出貨 - 主管要求優先追蹤

### 10. Metrics

績效資訊： - Completed Tasks - Average Response Time - Tool Usage -
Token Cost - Success Rate

## Spec / Status 分離

### Employee Spec

相對固定： - Identity - Brain - Role - Capability - Permission -
Resources

### Employee Status

持續變化： - State - Inbox - Current Task - Commitments - Working
Memory - Metrics

此設計有利於版本管理、快照、部署與模板化。

## Template 與 Instance

建議分為：

``` text
Employee Template
    ↓
Employee Instance
```

例如：

-   Steve（Template）
    -   Steve@台灣廠
    -   Steve@南京廠
    -   Steve@越南廠

共用： - Brain - Role - Capability

各自擁有： - Inbox - Commitments - Memory - KPI

## 設計原則

1.  Brain 是知識，不是員工。
2.  Employee 是 Runtime 管理的核心物件。
3.  Role 定義責任，Brain 定義知識。
4.  Capability 定義能力，Resources 提供工具。
5.  Inbox 管理短期工作。
6.  Commitments 管理長期責任。
7.  State 管理生命週期。
8.  Spec 與 Status 分離。
9.  Template 與 Instance 分離，支援企業級部署。
