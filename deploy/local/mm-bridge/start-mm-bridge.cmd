@echo off
rem mm-bridge 形狀轉譯 shim 啟動器——gbrain multimodal（litellm recipe）→ llama-server
rem 前提：llama-server 已帶 --mmproj 在 127.0.0.1:8080 跑 embeddinggemma-2
rem 開機自啟：shell:startup 放本檔的捷徑，或排程任務
cd /d "%~dp0"
bun mm-bridge.mjs
