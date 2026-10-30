# 文件工廠

> 適用版本：v0.5.11。工廠把原始文件變成結構化知識頁，寫進作用中腦的來源 repo——知識入庫的主要入口。

## 基本流程

1. 選好作用中腦與**作用中來源**（沒有來源時頁面會提示先到「腦」頁新增）。
2. 把檔案**拖放**到對應工廠（或點拖放區選檔）。
3. 自動轉換並寫入——「轉換並寫入中（LLM 結構化可能需數秒）…」。
4. 點選已處理檔案可**預覽**產生頁，逐頁檢查、編輯後「覆蓋此頁」。
5. 按**「Sync 到腦」**（`sync --source <id>`，含 embed/extract）——至此知識才可被檢索。

「設定 → 應用程式」可開「工廠寫檔後自動 commit + sync」，省掉第 5 步（無 git remote 的腦建議同時開 `--no-pull`）。

## 工廠類型

Schema pack **v1** 的內建工廠：

| 工廠 | 接受 | 寫入 |
|---|---|---|
| **圖片** | jpg/png 等圖檔 | `media/`＋強制註解頁（見下） |
| **people** | CSV（Google Contacts 匯出） | `people/` |
| **companies** | txt / md / pdf | `companies/` |
| **meeting** | txt / md / pdf | `meetings/` |
| **projects** | txt / md / pdf | `projects/` |
| **concepts** | txt / md / pdf | `concepts/` |
| **inbox（速記）** | txt / md | `inbox/`（gbrain capture；**不進圖譜**——速記不做實體連結） |
| **自動** | csv / txt / md / pdf | 依內容判斷分派到各類型；無法判定時跳出確認讓你選 |

**schema pack v2**（15 個標準型別）可用「設定 → 模型」的「執行 unify-types」遷移（新增 media、tweet、deal、email、analysis 等型別）；新腦建議直接用 v2，既有 v1 腦遷移前先確認沒有 gbrain 行程在跑並備份腦資料庫。

### 圖片工廠

圖片**必須填註解**才會入庫（「每張圖都要填註解」）：每張圖存進 `media/` 並生成同名註解頁，你的描述就是頁面內容。sync 時 gbrain 會做多模態嵌入與 `image_of` 連結——前提是嵌入服務啟用了多模態（`--mmproj`，見[第 02 章](02-requirements.md)）；未啟用時圖片在 sync 會被略過並提示。

## 複雜 PDF 怎麼被處理

PDF 依訊號自動分流（`pdf_convert_policy`：`auto`／`always_fast`／`always_mineru`）：

- **簡單 PDF**（純文字、單欄、無圖表）——快速路徑直接抽取。
- **複雜 PDF**（雙欄論文、掃描、含圖）——經 **MinerU** 完整轉換：章節切塊＋圖片筆記＋向量，圖片成為獨立檢索項。轉換嘗試過程記錄在來源 repo 的 `conversion-report.json`，可稽核。

沒裝 MinerU 時複雜 PDF 品質下降（見[第 02 章](02-requirements.md)的降級表）；egress 信任分層（文件會不會送去第三方雲解析）見[第 13 章](13-governance.md)。

## 直接新增頁面

每個工廠右上角「+」：不用檔案，直接在編輯器以合規 template 新增頁面（例如一筆人物資料）。存檔時 LLM 會把你寫的人名／公司名補成 `[[wikilink]]`、以 title 作為檔名；「取名寫入」或「儲存並 Sync」一步完成。

企業模式（Enterprise C′）新增時還可指定**保密等級**與**圈子（部門／專案）**——寫入更高的等級需要作者具備對應 clearance。

## 產出統計

轉換完成的摘要會列出：寫入頁數、凍結未覆寫（既有頁保護——不自動覆蓋人工編輯過的內容）、丟棄非法日期事件、LLM 富集成功／失敗數。people 工廠另有「重建 companies」：從 people 掃出公司／組織，重建 `companies/` 頁與連結（可加 `--clean` 清除重建）。

---
[← 08 員工管理](08-employees.md)｜[索引](README.md)｜[10 待辦與事件 →](10-inbox-and-events.md)
