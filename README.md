# Codex Switch Lite

A tiny macOS switcher for exactly two Codex Desktop modes:

- **ChatGPT Plus** — uses your existing official Codex/ChatGPT OAuth login.
- **External API** — routes model inference to `https://code.adinsolution.link` using model `gpt-5.5`.

This is deliberately much smaller than CC Switch. It follows CC Switch's modern **config-only third-party switching** model: `auth.json` is never modified; the external API key is injected into the external provider in `config.toml` as `experimental_bearer_token` only while External mode is active.

## What it does not touch

- `~/.codex/auth.json` — never read or written, except checking whether the file exists for status display.
- Your MCP, projects, features, reasoning, permissions, model catalog path, AGENTS settings, and unrelated provider definitions — preserved.
- `CODEX_HOME` — no second Codex home.

## Files it manages

```text
~/.codex/config.toml                         # active Codex config
~/.codex/codex-switch-lite/plus.toml        # Plus mode profile
~/.codex/codex-switch-lite/external.toml    # External mode profile (key stripped)
~/.codex/codex-switch-lite/state.json       # current mode
~/.codex/codex-switch-lite/backups/         # automatic backups
macOS Keychain: CodexSwitchLite.ExternalAPI # external API key
```

If the old Skill profiles exist under `~/.codex/external-api-switcher/`, the Lite app imports them on first run where possible.

## Build on Mac

Double-click `build-mac.command`, or in Terminal:

```bash
cd codex-switch-lite
./build-mac.command
```

Requirements:

- macOS
- Xcode Command Line Tools
- Node.js 20+
- Rust stable
- pnpm

The script will install `pnpm` with Corepack if needed, install project dependencies, and run the Tauri build.

Built app:

```text
src-tauri/target/release/bundle/macos/Codex Switch Lite.app
```

Copy it to `/Applications` if you want.

## How to use

1. Open **Codex Switch Lite**.
2. On first run, click **初始化配置**. It snapshots/imports your current Plus/API configs without touching OAuth.
3. Enter your external API key once and click **保存 Key**. The key goes into macOS Keychain.
4. Click **切到 External API**. The app backs up config, switches provider, and restarts Codex Desktop.
5. In Codex Desktop, start a **new chat**. That new thread uses the external provider.
6. Click **切回 ChatGPT Plus** when you want official subscription inference again. Codex restarts; start a new chat.

## Routing used by External mode

```toml
model_provider = "ExternalAPI"
model = "gpt-5.5"
review_model = "gpt-5.5"

[model_providers.ExternalAPI]
name = "External API"
base_url = "https://code.adinsolution.link"
wire_api = "responses"
experimental_bearer_token = "<key from macOS Keychain>"
requires_openai_auth = true
```

`requires_openai_auth = true` deliberately keeps Codex Desktop aware of your official login, while the provider-scoped bearer token is used for the actual external inference request. This mirrors CC Switch's official-login preservation approach.

## Safety behavior

Every switch:

1. Saves the active mode profile.
2. Syncs common/non-provider settings into the target profile.
3. Validates TOML.
4. Creates a timestamped backup.
5. Atomically replaces `~/.codex/config.toml`.
6. Restarts Codex Desktop.
7. Rolls back automatically if the write/validation step fails.

The app keeps at most 20 automatic backups.

## Important

A provider change cannot turn an already-created Codex thread into a different provider. After switching/restarting, **create a new chat**.
