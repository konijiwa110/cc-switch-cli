# OpenClaw Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add OpenClaw support that keeps backend semantics close to `.upstream`, exposes only necessary TUI surfaces, and preserves maintainability in the Rust TUI codebase.

**Architecture:** Treat OpenClaw as a second additive-mode app alongside OpenCode. Keep the OpenClaw live-config read/write logic in a dedicated `openclaw_config.rs` module modeled on upstream behavior, then thread `AppType::OpenClaw` through provider services, prompt/settings paths, and the minimal TUI provider-management flow. Do not build rich OpenClaw-only TUI pages unless the backend capability requires a user-visible entry point.

**Tech Stack:** Rust, `serde`, `serde_json`, `json5`, `json_five`, ratatui TUI, Rust unit tests, Rust integration tests

---

### Task 1: Lock the OpenClaw app skeleton with failing tests first

**Files:**
- Modify: `src-tauri/tests/app_type_parse.rs`
- Modify: `src-tauri/tests/app_config_load.rs`
- Modify: `src-tauri/src/app_config.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/prompt_files.rs`
- Modify: `src-tauri/src/sync_policy.rs`

**Step 1: Write the failing app type parse tests**

Add tests that assert:

```rust
assert_eq!(AppType::from_str("openclaw").unwrap(), AppType::OpenClaw);
assert!(AppType::all().any(|app| app == AppType::OpenClaw));
assert!(AppType::OpenClaw.is_additive_mode());
```

**Step 2: Run the focused parse tests and watch them fail**

Run:

```bash
cargo test --test app_type_parse
```

Expected: FAIL because `OpenClaw` is not yet part of `AppType`.

**Step 3: Write the failing config load/default tests**

Add tests that prove:

- default config contains an `openclaw` manager
- prompt roots include `openclaw`
- settings can round-trip an `openclawConfigDir`

**Step 4: Run the focused config tests and watch them fail**

Run:

```bash
cargo test --test app_config_load
```

Expected: FAIL because the default config and settings do not yet include OpenClaw.

**Step 5: Implement the minimal skeleton changes**

Add `AppType::OpenClaw`, include it in `PromptRoot`, `CommonConfigSnippets`, `AppType::all()`, `FromStr`, additive-mode checks, settings override-dir handling, prompt path resolution, and sync policy wiring.

### Task 2: Add a dedicated OpenClaw config module close to upstream

**Files:**
- Create: `src-tauri/src/openclaw_config.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/provider.rs`

**Step 1: Write the failing OpenClaw config tests**

Create unit tests in `src-tauri/src/openclaw_config.rs` that prove:

- missing `~/.openclaw/openclaw.json` loads a default object with `models.providers = {}`
- `set_provider()` writes a provider under `models.providers.<id>`
- `remove_provider()` only removes the target provider
- typed round-trip preserves `baseUrl`, `apiKey`, `api`, `models`, and unknown fields

**Step 2: Run the focused module tests and watch them fail**

Run:

```bash
cargo test openclaw_config
```

Expected: FAIL because the module does not exist yet.

**Step 3: Implement the OpenClaw config types and path helpers**

Model the code closely on upstream:

- `get_openclaw_dir()`
- `get_openclaw_config_path()`
- `read_openclaw_config()`
- `get_providers()` / `get_typed_providers()`
- `set_provider()` / `set_typed_provider()`
- `remove_provider()`

Prefer preserving JSON5 compatibility and avoid inventing a second config schema.

**Step 4: Keep the file focused**

If the module grows too much, split only low-level round-trip helpers into a private sibling module; do not smear OpenClaw logic into unrelated service files.

### Task 3: Generalize additive-mode provider behavior to include OpenClaw

**Files:**
- Modify: `src-tauri/src/services/provider/live.rs`
- Modify: `src-tauri/src/services/provider/mod.rs`
- Modify: `src-tauri/tests/provider_service.rs`
- Modify: `src-tauri/tests/import_export_sync.rs`
- Modify: `src-tauri/tests/opencode_provider.rs`

**Step 1: Write the failing provider-service tests**

Add focused tests that prove:

- importing default OpenClaw config pulls providers from `openclaw.json`
- `ProviderService::switch()` for OpenClaw writes the selected provider into live config without setting a current provider id
- `ProviderService::sync_current_to_live()` syncs all OpenClaw providers
- deleting an OpenClaw provider removes it from live config

**Step 2: Run the focused tests and watch them fail**

Run:

```bash
cargo test --test provider_service openclaw
cargo test --test import_export_sync openclaw
```

Expected: FAIL because additive-mode logic only handles OpenCode.

**Step 3: Extend live snapshots and live reads/writes**

Teach `services/provider/live.rs` to capture and restore OpenClaw live config snapshots.

**Step 4: Replace OpenCode-only additive branches with additive-by-app behavior**

Update provider service code paths so additive-mode behavior dispatches to:

- OpenCode backend for `AppType::OpenCode`
- OpenClaw backend for `AppType::OpenClaw`

Do this in:

- live import
- live read
- live write
- common snippet extraction
- delete/remove-from-live
- takeover/backup guards

**Step 5: Keep OpenCode green**

Re-run the existing OpenCode integration tests after each OpenClaw additive-mode change.

### Task 4: Add OpenClaw provider form support with minimal TUI fields

**Files:**
- Modify: `src-tauri/src/cli/tui/form.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state_loading.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_json.rs`
- Modify: `src-tauri/src/cli/tui/ui/forms/provider.rs`
- Modify: `src-tauri/src/cli/tui/form/tests.rs`

**Step 1: Write the failing form tests**

Add tests that prove:

- `ProviderAddFormState::new(AppType::OpenClaw)` exposes only the OpenClaw fields you intend to support
- `to_provider_json_value()` writes `baseUrl`, `apiKey`, `api`, and a model list/object without dropping unknown fields
- `from_provider()` round-trips existing OpenClaw provider JSON back into the form

**Step 2: Run the focused form tests and watch them fail**

Run:

```bash
cargo test cli::tui::form::tests::openclaw
```

Expected: FAIL because the form does not know about OpenClaw.

**Step 3: Implement the smallest useful field set**

Support only fields that are necessary for provider management in TUI, for example:

- base URL
- API key
- API mode/name
- at least one model id/name entry

Do not add OpenClaw-only env/tools/agents editors to the provider form.

**Step 4: Preserve unknown provider data**

When editing an existing OpenClaw provider, keep non-form JSON fields in `extra` so a save does not flatten user config.

### Task 5: Thread OpenClaw through TUI navigation and minimal views

**Files:**
- Modify: `src-tauri/src/cli/tui/app/helpers.rs`
- Modify: `src-tauri/src/cli/tui/app/menu.rs`
- Modify: `src-tauri/src/cli/tui/data.rs`
- Modify: `src-tauri/src/cli/tui/theme.rs`
- Modify: `src-tauri/src/cli/tui/ui/chrome.rs`
- Modify: `src-tauri/src/cli/tui/ui/overlay/basic.rs`
- Modify: `src-tauri/src/cli/tui/ui/tests.rs`
- Modify: `src-tauri/src/cli/tui/app/tests.rs`

**Step 1: Write the failing TUI navigation/render tests**

Add tests that prove:

- app cycling includes OpenClaw
- header tabs render OpenClaw
- app picker includes OpenClaw
- provider page can initialize under OpenClaw without panicking

**Step 2: Run the focused TUI tests and watch them fail**

Run:

```bash
cargo test cli::tui::app::tests::tests::openclaw
cargo test cli::tui::ui::tests::openclaw
```

Expected: FAIL because the TUI only knows Claude/Codex/Gemini/OpenCode.

**Step 3: Wire OpenClaw into app tabs and data loaders**

Keep the UX minimal:

- add an OpenClaw tab
- let provider/config pages load under OpenClaw
- avoid introducing new complex panes unless a backend action has to be surfaced

**Step 4: Choose explicit unsupported behavior instead of fake support**

Where OpenClaw is not yet supported in TUI, show a short static message or disable the action instead of routing into a broken flow.

### Task 6: Seal unsupported surfaces and align behavior with upstream

**Files:**
- Modify: `src-tauri/src/services/proxy.rs`
- Modify: `src-tauri/src/services/stream_check.rs`
- Modify: `src-tauri/src/services/mcp.rs`
- Modify: `src-tauri/src/cli/tui/runtime_actions/helpers.rs`
- Modify: `src-tauri/src/cli/tui/runtime_actions/mcp.rs`
- Modify: `src-tauri/tests/proxy_service.rs`

**Step 1: Write the failing unsupported-behavior tests**

Add tests that prove OpenClaw:

- does not participate in proxy takeover flows
- does not expose stream check as if it were supported
- does not pretend to manage MCP when upstream semantics say otherwise

**Step 2: Run the focused tests and watch them fail**

Run:

```bash
cargo test --test proxy_service openclaw
```

Expected: FAIL or compile gaps because OpenClaw is not handled explicitly.

**Step 3: Implement explicit no-op / unsupported branches**

Mirror upstream behavior as closely as possible and keep the UI honest.

### Task 7: Full verification and cleanup

**Files:**
- Verify: `src-tauri/src/openclaw_config.rs`
- Verify: `src-tauri/src/services/provider/mod.rs`
- Verify: `src-tauri/src/cli/tui/form/*.rs`
- Verify: `src-tauri/src/cli/tui/ui/*.rs`

**Step 1: Run targeted tests during each task**

Keep the red-green cycle tight; do not wait for the full suite until the end.

**Step 2: Run formatting**

Run:

```bash
cargo fmt
```

**Step 3: Run the full test suite**

Run:

```bash
cargo test
```

**Step 4: Run clippy and fix warnings introduced by this feature**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

**Step 5: Re-check scope against the three user principles**

Verify that:

- backend behavior stays close to `.upstream`
- TUI only exposes necessary OpenClaw interactions
- no touched file grows needlessly when a focused extraction would keep it maintainable

**Step 6: Do not commit**

Per repo instructions, do not create a git commit unless the user explicitly asks.
