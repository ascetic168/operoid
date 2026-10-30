# 附錄 A：GBrain 架構說明

> 本章撰寫中。目標讀者：第一次接觸 GBrain 的新手。

## 預定內容

- GBrain 是什麼：Operoid 的「大腦」——知識儲存、結構化與檢索引擎（外部黑盒，經 MCP／CLI 被員工取用）
- 為什麼 Operoid 需要 GBrain：Runtime 管執行、不管推理；員工回答公司事務需要**有出身、有邊界**的知識
- 與「一般單獨 RAG」的差異：不只是 chunk→embed→top-k——wiki 頁面＋知識圖譜＋向量檢索三路融合、圖表向量、VLM 圖說；檢索前授權與收據
- 與「Wiki 搜尋」的差異：關鍵字全文只能找「寫過的詞」；GBrain 能回答「沒有寫在一起的事」
- 腦的結構：來源（git repo）、頁面、圖譜、向量；sync／embed／extract 流程
- 在 Operoid 哪裡碰到它：腦頁、工廠、知識庫頁、員工工具、Claude Code
- 新手常見誤解 FAQ＋延伸閱讀

---
[← 18 疑難排解](18-troubleshooting.md)｜[索引](README.md)｜[附錄 B →](appendix-b-development.md)
