---
name: interview-prep
description: Create structured interview plans with competency-based questions and scorecards. Trigger with "interview plan for", "interview questions for", "how should we interview", "scorecard for", or when the user is preparing to interview candidates.
---

> 【環境註記】本文改編自 Anthropic《knowledge-work-plugins》開源庫（Apache-2.0，出處見本外掛目錄 SOURCE.md）。原文假設 Claude Cowork 環境：Artifacts／Pages／Slides 等呈現面改以本環境可用的檔案工具產出；`/plugin:command` 斜線指令改為讀取同外掛對應技能目錄的 SKILL.md；CRM／日曆／email 等連接器不存在時，依技能內建的檔案 fallback 進行。

# Interview Prep

Create structured interview plans to evaluate candidates consistently and fairly.

## Interview Design Principles

1. **Structured**: Same questions for all candidates in the role
2. **Competency-based**: Map questions to specific skills and behaviors
3. **Evidence-based**: Use behavioral and situational questions
4. **Diverse panel**: Multiple perspectives reduce bias
5. **Scored**: Use rubrics, not gut feelings

## Interview Plan Components

### Role Competencies
Define 4-6 key competencies for the role (e.g., technical skills, communication, leadership, problem-solving).

### Question Bank
For each competency, provide:
- 2-3 behavioral questions ("Tell me about a time...")
- 1-2 situational questions ("How would you handle...")
- Follow-up probes

### Scorecard
Rate each competency on a consistent scale (1-4) with clear descriptions of what each level looks like.

### Debrief Template
Structured format for interviewers to share findings and make a decision.

## Output

Produce a complete interview kit: panel assignment (who interviews for what), question bank by competency, scoring rubric, and debrief template.