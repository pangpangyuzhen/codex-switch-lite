# What was kept from CC Switch, and what was removed

## Kept

- Codex-only provider switching concept.
- Official-login preservation: `auth.json` stays official.
- Third-party inference key is provider-scoped in `config.toml` as `experimental_bearer_token`.
- `requires_openai_auth = true` in preservation mode so Codex Desktop stays logged in while the provider token handles external inference.
- Restart Codex after provider changes.
- Backup + validation before live config replacement.

## Removed

- Claude Code / Claude Desktop / Gemini / OpenCode / other clients.
- Provider marketplace and provider list management.
- Local proxy and protocol conversion.
- MCP, Skills, prompt, session and usage management.
- Multiple third-party endpoints.
- Cloud/WebDAV/S3 sync.
- Update center and all ancillary settings.

## Added for this Lite build

- Exactly two explicit profiles: `plus.toml` and `external.toml`.
- External endpoint fixed to `https://code.adinsolution.link`.
- External model fixed to `gpt-5.5`.
- External key stored in macOS Keychain and only injected into the live config while External mode is active.
- Automatic import of the previous `~/.codex/external-api-switcher/` profile files when present.
- Common Codex settings are synchronized between the two profiles on switch while mode-owned fields remain isolated.
