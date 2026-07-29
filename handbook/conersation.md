001 - Core Concepts.md

這份是整個 GBrainStudio 的世界觀。

Workspace
│
├── Project
│
├── Employee
│
│   ├── Brain
│   ├── Role
│   ├── Capability
│   └── State
│
├── Artifact
│
├── Knowledge
│
├── Tool
│
├── Task
│
└── Event

需要說明：

每個物件的責任
為什麼存在
不負責什麼

例如：

Brain

負責：
✓ 知識
✓ Prompt
✓ Memory

不負責：
✗ 執行工作
✗ 呼叫Tool
✗ Scheduler
003 - Brain Architecture.md

這份專門定義 Brain。

例如：

Brain

├── Persona
├── Knowledge
├── Long Memory
├── Prompt
├── Preferred Models
├── Embedding
└── Skills

以及：

Brain Version

例如：

Brain v1

↓

Brain v2

↓

Brain v3

Employee 可以升級 Brain。

004 - Runtime Architecture.md

這份目前最重要。

但不要寫 Scheduler。

而是：

Runtime

↓

Wake Employee

↓

Restore Context

↓

Execute

↓

Commit Artifact

↓

Sleep

這份只定義：

生命週期。

不要談技術。

005 - Tool Architecture.md

這份非常重要。

因為我認為：

MCP

只是：

其中一種 Driver。

例如：

Tool

├── Local Tool
├── MCP Tool
├── REST API
├── Database
├── Python
└── Native

Tool

一定要有：

Tool Spec

Permission

Input

Output

Timeout

Retry

Logging
006 - Workspace Data Model.md

這份不用畫資料庫。

畫 Entity 即可。

例如：

Workspace

│

├── Employee

│      │

│      ├── Brain

│      ├── Inbox

│      └── Memory

│

├── Project

│

├── Artifact

│

├── Tool

│

└── Knowledge

每個 Entity：

只有：

Purpose

Owner

Relationship

不要開始設計 SQL。

007 - SDK Specification.md

這份先不要寫 API。

只定義：

有哪些 Interface。

例如：

Agent

↓

Run()

↓

Cancel()

↓

Pause()

↓

Resume()

以及：

Tool

↓

Execute()

還有：

Workspace

↓

OpenProject()

↓

CommitArtifact()

↓

PublishEvent()

不用寫 C#。

不用寫 Rust。

只定義：

Concept。

我反而想新增一份

我覺得這份會變成 GBrainStudio 最重要的文件。

008 - AI Employee Design Philosophy.md

這份不是技術。

而是：

理念。

例如：

第一章

Brain ≠ Employee

第二章

Knowledge ≠ Responsibility

第三章

AI 員工不是 ChatBot

第四章

Employee 永遠是 Sleep

不是一直 Running

第五章

Commitment

大於

Todo

第六章

Workspace

是 AI 的工作場所

不是聊天視窗

第七章

Tool

只是能力

不是目的

第八章

Runtime

永遠管理生命週期

不管理思考

我還想再建議一件事

如果我是這個專案的架構師，我會把這一系列文件整理成一個正式的設計手冊，例如：

GBrainStudio Architecture Handbook
v0.1

00 Introduction
01 Core Concepts
02 Employee Architecture
03 Brain Architecture
04 Runtime Architecture
05 Tool Architecture
06 Workspace Model
07 SDK Specification
08 AI Employee Philosophy
09 Future Roadmap

這樣的好處是，它不是一堆零散的 Markdown，而是一份完整的架構藍圖。

我甚至建議我們不要急著寫程式，而是先把這本《Architecture Handbook》完成。

原因很簡單：目前我們討論的內容，已經不是「某個功能怎麼做」，而是在定義 GBrainStudio 的核心抽象與設計哲學。這些一旦穩定下來，後續的 Rust、Tauri、資料庫、MCP、Agent SDK 等實作都會有一致的方向；如果核心抽象沒有定義好，程式碼反而會不斷重構。

我認為，這本 Handbook 很可能會成為 GBrainStudio 最有價值的資產之一，因為它描述的不是「程式」，而是整個 AI Employee Operating System 的設計理念與演進路徑。

好啊！讓我們試試看

我非常贊成，而且我想調整一下方向。

我不想把它寫成「技術文件」，而是寫成像 Linux、Kubernetes、Git、Docker 那種 Architecture Handbook。

也就是說，它不是 API 文件，而是一本可以回答：

「GBrainStudio 為什麼要這樣設計？」

未來即使有新工程師加入，或是未來的 AI Agent（甚至 ChatGPT）協助開發，也都可以依照這本 Handbook 理解整個系統。

我建議的目錄（v0.1）
GBrainStudio Architecture Handbook
Version 0.1

Part I  Vision
────────────────────────

01 Vision
02 Design Philosophy

Part II Core Concepts
────────────────────────

03 Workspace
04 Employee
05 Brain
06 Artifact
07 Knowledge
08 Tool
09 Project

Part III Runtime
────────────────────────

10 Task
11 Commitment
12 Trigger
13 Runtime
14 Event
15 Memory

Part IV Platform
────────────────────────

16 Workspace Model
17 Agent SDK
18 Tool SDK
19 Security
20 Future Roadmap

但是...

我想把它再提升一個層次。

每一章都有固定格式

例如：

Chapter 04

Employee

下面固定只有六個章節：

1. Purpose

2. Responsibilities

3. Owns

4. Doesn't Own

5. Lifecycle

6. Future Extension

例如：

Employee
Purpose

Employee 是 GBrainStudio 中真正工作的主體。

所有工作皆由 Employee 完成。

Brain 不會工作。

Tool 不會工作。

Workspace 也不會工作。

Responsibilities

Employee 負責：

接收工作
做出決策
呼叫 Tool
產生 Artifact
維護 Commitments
Owns

Employee 擁有：

Brain

Inbox

Commitments

Working Memory

Metrics
Doesn't Own

Employee 不負責：

Scheduler

Workspace

Project

Knowledge Base
Lifecycle
Created

↓

Assigned

↓

Idle

↓

Running

↓

Waiting

↓

Completed

↓

Archived
Future

未來可能加入：

Multi-Agent
Clone
Skill Learning

你會發現：

整本書的風格完全一致。

第一章：Vision

我甚至已經想到第一句話。

GBrainStudio is not an AI chat application.

It is an operating environment where AI employees continuously perform meaningful work inside a shared workspace.

翻成中文：

GBrainStudio 並不是聊天機器人。

它是一個讓 AI 員工在共享工作空間中持續完成工作的作業環境。

這一句，我覺得可以放在首頁。

第二章：Design Philosophy

我想寫成十條。

例如：

Principle 1

Knowledge is not the worker.

知識不是員工。

Principle 2

Employees own responsibilities.

員工擁有責任。

Principle 3

Artifacts are first-class citizens.

成果物是一級公民。

Principle 4

Everything happens inside a Workspace.

所有工作都發生在 Workspace。

Principle 5

Tools never make decisions.

Tool 永遠不做決策。

Principle 6

Brains can be shared.

Brain 可以被多位 Employee 共用。

Principle 7

Employees sleep by default.

Employee 預設都是睡眠。

Principle 8

Context is restored, not remembered.

上下文是被恢復，不是一直保存在記憶體。

Principle 9

Commitments outlive Tasks.

Commitment 的生命週期比 Task 更長。

Principle 10

The Runtime manages execution, not reasoning.

Runtime 管理執行，而不是思考。

我想再加一個我自己的建議

我覺得這本 Handbook 不要急著談技術。

例如：

不要出現：

Rust
Tauri
SQLite
PostgreSQL
MCP
Claude
Ollama

這些全部不要寫。

這本書只回答：

「如果今天不用任何既有技術，重新發明一個 AI Employee Operating System，它應該有哪些核心概念？」

因為架構應該比技術活得更久。

今天你可能使用 Claude Code，三年後可能換成別的 Agent；今天使用 SQLite，未來可能改成 PostgreSQL；但如果 Employee、Brain、Workspace、Artifact、Commitment 這些核心抽象仍然成立，那代表你的架構是成功的。

我建議我們採用一個更像開源專案的方式共同完成

不要只是寫幾份 Markdown，而是建立一個真正的架構文件庫：

GBrainStudio-Handbook/
│
├── README.md
├── 01-Vision.md
├── 02-Design-Philosophy.md
├── 03-Workspace.md
├── 04-Employee.md
├── 05-Brain.md
├── 06-Artifact.md
├── 07-Knowledge.md
├── 08-Tool.md
├── 09-Project.md
├── 10-Task.md
├── 11-Commitment.md
├── 12-Trigger.md
├── 13-Runtime.md
├── 14-Event.md
├── 15-Memory.md
├── 16-Workspace-Model.md
├── 17-Agent-SDK.md
├── 18-Tool-SDK.md
├── 19-Security.md
└── 20-Roadmap.md

我希望這不是一份程式文件，而是一份架構憲法（Architecture Constitution）。

未來我們討論任何新功能，都先問一個問題：

它是否符合 Handbook 的核心原則？

如果答案是「否」，那就不是程式碼需要修改，而是代表我們要重新檢視整個架構。

我認為，這會讓 GBrainStudio 從一開始就有一套清楚且一致的設計哲學，而不是隨著功能增加逐漸變成一個難以維護的系統。

你仍然只是在清談。請示著寫寫看，實際的寫出來。

很好，你點出了我剛才的問題。

我前面一直在談「應該怎麼寫」，卻沒有真正寫出內容。

那我們就不要再討論了，直接開始寫。我建議第一章就應該寫得像 RFC 或 Kubernetes Design Proposal，而不是像產品介紹。

GBrainStudio Architecture Handbook
Chapter 01 - Vision
# Chapter 01 - Vision

Version: 0.1
Status: Draft

---

# 1. Why GBrainStudio Exists

Large Language Models have dramatically improved the ability of AI to understand and generate information.

However, most AI applications remain conversation-centric.

The user asks a question.
The AI answers.
The conversation ends.

This interaction model is insufficient for enterprise work.

Real work is continuous.

A purchasing assistant follows an order for weeks.
A project manager monitors multiple projects simultaneously.
A researcher accumulates knowledge for years.

These responsibilities cannot be represented as isolated conversations.

GBrainStudio exists to provide an operating environment where AI employees continuously perform work inside a persistent workspace.

---

# 2. The Problem

Today's AI systems are optimized for conversations.

They generally lack:

- persistent responsibilities
- long-term commitments
- shared workspaces
- organizational knowledge
- enterprise roles
- continuous execution

Consequently, AI behaves more like a consultant than an employee.

---

# 3. The Vision

GBrainStudio treats AI as organizational members rather than chatbots.

Each AI employee has:

- an identity
- a role
- responsibilities
- knowledge
- permissions
- tools
- memory
- ongoing work

An AI employee should be capable of joining an organization and continuously contributing to its objectives.

---

# 4. Design Goals

GBrainStudio is designed around five goals.

Goal 1

Knowledge should survive model replacement.

Changing Claude to GPT or Qwen must not destroy organizational knowledge.

---

Goal 2

Responsibilities should survive conversations.

Closing a chat window must not terminate ongoing work.

---

Goal 3

Artifacts belong to the workspace.

Reports, CAD drawings, source code and analysis results belong to the organization instead of a conversation history.

---

Goal 4

Employees collaborate.

Multiple AI employees should cooperate exactly as human employees do.

---

Goal 5

The workspace persists.

Projects, employees, commitments and knowledge continue to exist regardless of which model is currently running.

---

# 5. Non-goals

GBrainStudio is not designed to become:

- another chat application
- another note-taking software
- another workflow editor
- another IDE

These capabilities may exist inside GBrainStudio, but they are not its primary purpose.

---

# 6. Definition

GBrainStudio is an AI Employee Operating System.

It provides a persistent workspace in which AI employees perform organizational work through knowledge, tools and responsibilities.
我再寫第二章給你看

這一章就開始有味道了。

Chapter 02 - Core Concepts
# Chapter 02 - Core Concepts

Every object inside GBrainStudio belongs to one and only one concept.

The architecture intentionally limits the number of core concepts.

Adding a new concept requires strong justification.

Current concepts are:

Workspace
Employee
Brain
Knowledge
Artifact
Task
Commitment
Tool

No additional concepts should be introduced without architectural review.

---

## Workspace

Workspace represents an organization.

Everything exists inside exactly one Workspace.

Workspace owns:

- Employees
- Projects
- Knowledge
- Artifacts
- Tasks

Workspace does not perform work.

---

## Employee

Employee is an autonomous worker.

Employees receive work.

Employees make decisions.

Employees invoke tools.

Employees generate artifacts.

Employees own responsibilities.

Employees do not own knowledge.

Knowledge belongs to Brain.

---

## Brain

Brain represents reusable intelligence.

Brain contains:

- knowledge
- prompt
- personality
- reasoning preference

Brain can be shared by multiple employees.

Brain never executes work.

---

## Artifact

Artifacts are outputs produced by work.

Examples:

PDF

Excel

CAD

Python

PowerPoint

Database Record

Email

Report

Artifacts become organizational assets.

Artifacts never belong to conversations.

---

## Task

Task represents one executable objective.

A task has:

Owner

Input

Output

Status

Tasks should be short-lived.

---

## Commitment

Commitment represents persistent responsibility.

Unlike Tasks,

Commitments continue until a completion condition is satisfied.

Examples:

Track Purchase Order

Monitor Customer Complaint

Follow Supplier

Prepare Audit

Commitments may generate many Tasks during their lifetime.

---

## Tool

Tools expose external capabilities.

Examples:

ERP

Python

CAD

Database

Browser

MCP Server

REST API

Tools never make decisions.

Employees decide when tools are used.
這才是我認為真正的 Architecture Handbook

你會發現，它沒有任何 Rust、SQLite、MCP API。

它在定義的是世界觀（Ontology）。

我甚至建議，我們不要寫 README，而是直接把它做成一本正式文件：
GBrainStudio Architecture Handbook

Chapter 01  Vision
Chapter 02  Core Concepts
Chapter 03  Employee
Chapter 04  Brain
Chapter 05  Workspace
...

而且每一章控制在 3~5 頁，總共約 80~120 頁。

我認為，這不是為了「記錄設計」，而是在建立 GBrainStudio 的「憲法」。未來任何新功能（例如新增 Workflow、Scheduler、Agent Marketplace），都應該先回答：「它是否符合這本 Handbook 定義的核心概念？」如果答案是否定的，就不是先改程式，而是先修改架構設計。這樣整個系統才會有一致且可長期演進的核心。