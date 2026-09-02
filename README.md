# Codex Switch Lite

一个只为 **Codex Desktop** 做的轻量 Provider 切换工具。

## 用途

- 一键切换 **ChatGPT Plus / 官方 OpenAI OAuth**
- 一键切换 **External API**
- 保留现有 `~/.codex/auth.json`，不覆盖 Plus 登录
- 切换前自动备份 Codex 配置
- 不管理 Claude、Gemini、MCP、Skills 等无关功能

## 默认 External API

- Base URL: `https://code.adinsolution.link`
- Model: `gpt-5.5`
- Wire API: `responses`

> API Key 不包含在仓库或安装包配置中，请在应用内自行填写。

## 下载

macOS Universal 版本（Apple Silicon + Intel）：

`dist/Codex-Switch-Lite-0.1.0-universal.dmg`

首次打开若被 macOS Gatekeeper 拦截，可在 Finder 中右键应用 → **打开**。

## 说明

这是面向个人 Codex Desktop 工作流的轻量版本，思路参考 CC Switch 的 Codex Provider 切换方式，并针对单一 Codex 场景做了裁剪。

当前构建未做 Apple Developer ID 公证签名。
