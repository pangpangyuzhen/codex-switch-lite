# Codex task notes

This is a deliberately small Tauri 2 + React + Rust macOS app derived from the Codex switching/auth-preservation idea in CC Switch 3.20.1.

Do not add multi-provider marketplaces, proxy routing, MCP management, Skills management, usage analytics, cloud sync, or extra CLI support.

Core invariants:

- Never write `~/.codex/auth.json`.
- External inference must use provider-scoped `experimental_bearer_token` in `config.toml`.
- ChatGPT Plus mode uses built-in `model_provider = "openai"` and the existing official OAuth file.
- External mode uses `model_provider = "ExternalAPI"`, `https://code.adinsolution.link`, Responses API, model `gpt-5.5`.
- Keep Plus and External as explicit full mode profiles; sync unrelated/common settings on switch.
- Validate and back up before any live config replacement.
- Restart Codex Desktop after switching and tell the user to start a new chat.
