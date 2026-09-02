# Third-party notices

Codex Switch Lite is a Codex-only lightweight derivative inspired by the Codex provider/auth-preservation implementation in **CC Switch**:

- Upstream: https://github.com/farion1231/cc-switch
- Upstream version inspected: 3.20.1 (2026-08-28 release line)
- Upstream author: Jason Young / farion1231
- License: MIT (see `LICENSE`)

The important upstream behavior retained here is:

1. Keep the official ChatGPT/Codex OAuth cache in `~/.codex/auth.json` untouched.
2. Route third-party Codex inference through `~/.codex/config.toml` only.
3. Put the third-party key on the active provider as `experimental_bearer_token` rather than replacing `auth.json`.
4. Restart Codex after switching so the Desktop app reloads the provider configuration.

This Lite project intentionally removes the rest of CC Switch (Claude/Gemini/OpenCode providers, proxy, MCP/Skills manager, sessions, usage dashboard, cloud sync, etc.).
