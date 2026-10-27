---
name: compliance-tracking
description: Track compliance requirements and audit readiness. Trigger with "compliance", "audit prep", "SOC 2", "ISO 27001", "GDPR", "regulatory requirement", or when the user needs help tracking, preparing for, or documenting compliance activities.
---

> 【環境註記】本文改編自 Anthropic《knowledge-work-plugins》開源庫（Apache-2.0，出處見本外掛目錄 SOURCE.md）。原文假設 Claude Cowork 環境：Artifacts／Pages／Slides 等呈現面改以本環境可用的檔案工具產出；`/plugin:command` 斜線指令改為讀取同外掛對應技能目錄的 SKILL.md；CRM／日曆／email 等連接器不存在時，依技能內建的檔案 fallback 進行。

# Compliance Tracking

Help track compliance requirements, prepare for audits, and maintain regulatory readiness.

## Common Frameworks

| Framework | Focus | Key Requirements |
|-----------|-------|-----------------|
| SOC 2 | Service organizations | Security, availability, processing integrity, confidentiality, privacy |
| ISO 27001 | Information security | Risk assessment, security controls, continuous improvement |
| GDPR | Data privacy (EU) | Consent, data rights, breach notification, DPO |
| HIPAA | Healthcare data (US) | PHI protection, access controls, audit trails |
| PCI DSS | Payment card data | Encryption, access control, vulnerability management |

## Compliance Tracking Components

### Control Inventory
- Map controls to framework requirements
- Document control owners and evidence
- Track control effectiveness

### Audit Calendar
- Upcoming audit dates and deadlines
- Evidence collection timelines
- Remediation deadlines

### Evidence Management
- What evidence is needed for each control
- Where evidence is stored
- When evidence was last collected

### Gap Analysis
- Requirements vs. current state
- Prioritized remediation plan
- Timeline to compliance

## Output

Produce compliance status dashboards, gap analyses, audit prep checklists, and evidence collection plans.