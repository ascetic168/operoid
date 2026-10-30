# 系統需求

> 適用版本：v0.5.11。本章說明個人版（桌面）的平台、前置元件與能力降級；企業版伺服器的前置需求見 [DEPLOYMENT.md §2](../DEPLOYMENT.md)。

## 支援平台

| 平台 | 安裝檔 | 備註 |
|---|---|---|
| Windows | 安裝精靈（.exe）、.msi | x64；自動更新走 .exe 安裝格式 |
| macOS | .dmg | Apple Silicon 與 Intel 各有安裝檔 |
| Linux | .AppImage、.deb、.rpm | x86_64；需 `libwebkit2gtk-4.1` 等系統庫（見 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)） |

硬體方面沒有官方最低規格——任何能順暢執行現代桌面應用的電腦即可。知識檢索依賴的本機嵌入服務（見下節）是一個小型模型（EmbeddingGemma 2，768 維），常駐記憶體佔用不大；若加裝多模態投影器，BF16 版約多佔 0.9 GB、Q8_0 版約 0.5 GB。LLM 推理本身走你設定的雲端或本機端點，不在這台機器上跑。

## 必要前置元件

知識功能依賴三個外部元件，**全部自動偵測**，缺了會在介面上明說：

| 元件 | 用途 | 沒有它 | 安裝 |
|---|---|---|---|
| **git** | 同步流程會在更新圖譜前先 commit | 工廠寫檔後無法 sync | <https://git-scm.com/downloads> |
| **bun** | JS 執行環境；`gbrain` 透過 bun 安裝與執行 | gbrain 無法執行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 知識圖譜引擎 CLI | 無知識庫可查 | <https://github.com/garrytan/gbrain#cli-standalone-no-agent> |

路徑會自動偵測（Windows 為 `~/.bun/bin/gbrain.exe`），必要時可在「設定」頁覆寫（見[第 07 章](07-brains-and-models.md)）。

首次啟動時，若偵測到缺項，會出現「缺少前置程式」對話框，列出缺的元件、用途說明與安裝連結——裝好後重開 Operoid 即可。GBrain 是什麼、為什麼需要它，見[附錄 A](appendix-a-gbrain.md)。

## 知識管線選用元件

這些元件決定知識功能的**品質上限**，缺項不會阻斷流程（內建降級），但建議裝齊：

| 元件 | 用途 | 沒有它 |
|---|---|---|
| **llama-server**（llama.cpp，跑 EmbeddingGemma 2） | 本機嵌入後端——檢索的硬依賴，須常駐（預設 `127.0.0.1:8080`） | 檢索退為僅關鍵字比對 |
| **mmproj 投影器**（選配） | 圖片的「圖說＋原圖」聯合向量 | 圖片僅以圖說文字索引 |
| **MinerU**（選配，**強烈建議**；版本須 > 4.0） | 複雜 PDF 解析（雙欄／掃描／含圖）——`uv tool install "mineru>4.0"` | 僅能以快速模式處理非常簡單的 PDF（純文字、單欄、無圖表）；文件內圖片無法進入知識庫 |
| **VLM chat 端點**（取決於你設定的 chat 模型） | 生成端直接讀原圖作答 | 降級為「定位者」模式：答案指出哪張圖，像素內容由人類開圖確認 |

### 能力矩陣與警告橫幅

「設定 → 服務」頁有一張**能力矩陣**（git／bun／gbrain／MinerU／嵌入端點／多模態／VLM），安裝或調整設定後按「重新檢查」更新。缺項同時會以警告橫幅出現在主視窗上方，例如：

- 「未偵測到 MinerU——PDF 知識管線目前僅能以快速模式處理非常簡單的 PDF…」
- 「嵌入模型未啟用多模態——圖片僅以文字說明（caption）索引…」
- 「llama-server 缺 -b/-ub 8192 旗標——長 chunk 會被拒收…」

橫幅可按「關閉（本次執行不再顯示）」暫時收起；根治方式依橫幅說明安裝或調整啟動旗標。嵌入服務的完整啟動指令見 [DEPLOYMENT.md §10.1](../DEPLOYMENT.md) 與[第 18 章](18-troubleshooting.md)。

## 資料流向透明度

個人版的程序與資料都在本機：腦資料是本機檔案、嵌入走本機 `127.0.0.1`、無第三方遙測。**會離開這台機器的只有模型推論**，且端點皆由你自行設定（內建對話／文件轉換／Claude Code 三個通道各自送往哪裡，詳見 [DEPLOYMENT.md §11.4](../DEPLOYMENT.md)；治理面的說明見[第 13 章](13-governance.md)）。

---
[← 01 認識 Operoid](01-introduction.md)｜[索引](README.md)｜[03 安裝個人版 →](03-install-personal.md)
