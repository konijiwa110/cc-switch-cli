# OpenClaw Upstream Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring OpenClaw to near-upstream parity in the Rust/TUI app by giving it a distinct visual identity, aligning the TUI provider-management surface with upstream semantics, and matching upstream backend behavior as closely as practical.

**Architecture:** Keep OpenClaw as an additive-mode app, but stop treating it like a thin OpenCode clone. Use upstream `openclaw_config` and provider-service behavior as the source of truth for backend semantics, then make the TUI expose the same actions and fields in keyboard-first form instead of inventing a separate mental model. Prefer single-source helpers and shared state over scattering OpenClaw-specific conditionals across many files.

**Tech Stack:** Rust, ratatui, serde/serde_json, JSON5-compatible parsing/writing, Rust unit tests, Rust integration tests.

---

### Task 1: Give OpenClaw a distinct upstream-aligned visual identity

**Files:**
- Modify: `src-tauri/src/cli/tui/theme.rs`
- Modify: `src-tauri/src/cli/ui/colors.rs`
- Modify: `src-tauri/src/cli/tui/ui/tests.rs`

**Step 1: Write the failing theme tests**

Add focused tests that prove:

- `AppType::OpenClaw` uses a different accent than `AppType::OpenCode`
- `AppType::OpenClaw` uses a different accent than `AppType::Codex`
- the shared CLI/inquire colors for `OpenClaw` do not collapse to an existing app color

**Step 2: Run the focused tests and watch them fail**

Run:

```bash
cargo test openclaw_theme
```

Expected: FAIL because `OpenClaw` currently reuses `OpenCode` orange in ratatui and green in CLI highlight helpers.

**Step 3: Implement the smallest upstream-aligned color mapping**

Use a dedicated OpenClaw accent derived from upstream's rose/red brand direction instead of reusing orange or green. Change only the single-source color helpers so tabs, borders, selections, prompts, and highlights all move together.

**Step 4: Re-run the focused tests**

Run:

```bash
cargo test openclaw_theme
```

Expected: PASS.

### Task 2: Make the OpenClaw provider list use additive-mode semantics like upstream

**Files:**
- Modify: `src-tauri/src/cli/tui/data.rs`
- Modify: `src-tauri/src/cli/tui/app/content_entities.rs`
- Modify: `src-tauri/src/cli/tui/app/helpers.rs`
- Modify: `src-tauri/src/cli/tui/ui/providers.rs`
- Modify: `src-tauri/src/cli/tui/app/tests.rs`
- Modify: `src-tauri/src/cli/tui/ui/tests.rs`

**Step 1: Write the failing list/action tests**

Add tests that prove under `AppType::OpenClaw`:

- the provider list key bar no longer advertises `switch current provider`
- the primary action maps to `add/remove from live config`, not `set current`
- the UI exposes an explicit default-model action instead of a fake current-provider concept
- the list/detail surface does not expose unsupported stream-check shortcuts

**Step 2: Run the focused TUI tests and watch them fail**

Run:

```bash
cargo test cli::tui::app::tests::tests::openclaw
cargo test cli::tui::ui::tests::openclaw
```

Expected: FAIL because OpenClaw still inherits current-provider wording and actions.

**Step 3: Thread OpenClaw list state through data loading**

Add the minimum data needed to render upstream semantics in TUI:

- whether the provider exists in live config
- whether it is the default model/provider target
- the default or primary model id shown to users

Keep this data derivation centralized; do not recompute it ad hoc in multiple render functions.

**Step 4: Rework key bindings and labels**

Replace OpenClaw's `switch/current` vocabulary with upstream-style `add/remove/set default` vocabulary while keeping the TUI keyboard-first layout. Do not remove useful TUI-only affordances unless they contradict upstream semantics.

**Step 5: Re-run the focused tests**

Run the same commands from Step 2 and expect PASS.

### Task 3: Expand the OpenClaw provider form toward upstream field parity

**Files:**
- Modify: `src-tauri/src/cli/tui/form.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state_loading.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_json.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_templates.rs`
- Modify: `src-tauri/src/cli/tui/ui/forms/provider.rs`
- Modify: `src-tauri/src/cli/tui/form/tests.rs`

**Step 1: Write the failing form tests**

Add tests that prove OpenClaw forms now support the upstream-critical fields and rules:

- visible `provider_key` / id for add, immutable on edit
- protocol selection constrained to supported upstream values
- optional `User-Agent`
- multiple models with preserved order
- explicit default/fallback model selection
- no fake Common Config block for OpenClaw
- round-tripping existing provider JSON preserves unknown fields

**Step 2: Run the focused form tests and watch them fail**

Run:

```bash
cargo test cli::tui::form::tests::openclaw
```

Expected: FAIL because the current form only supports a minimal single-model subset.

**Step 3: Implement the upstream-critical fields first**

Match upstream behavior in TUI form terms:

- `providerKey`
- protocol picker
- API key / base URL
- optional user-agent
- ordered model list
- default/fallback model markers

Only add advanced per-model fields once the structure for multiple models is stable.

**Step 4: Align presets/templates with upstream intent**

Stop treating OpenClaw as `Custom` only. Add the minimum preset/template layer needed to mirror upstream-supported provider presets and suggested defaults without exploding the TUI into dozens of bespoke pages.

**Step 5: Re-run the focused form tests**

Run the same command from Step 2 and expect PASS.

### Task 4: Replace the simplified OpenClaw backend with upstream-like document semantics

**Files:**
- Modify: `src-tauri/src/openclaw_config.rs`
- Modify: `src-tauri/src/provider.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/tests/app_config_load.rs`

**Step 1: Write the failing config/document tests**

Add tests that prove:

- OpenClaw config round-trips JSON5-like documents without flattening unrelated sections
- provider updates do not rewrite unrelated `agents`, `env`, or `tools` sections
- removing a missing provider is a no-op instead of forcing a rewrite
- defaults/model sections can be read and written through typed helpers

**Step 2: Run the focused module tests and watch them fail**

Run:

```bash
cargo test openclaw_config
```

Expected: FAIL because current `openclaw_config.rs` is provider-centric and rewrites the whole file as strict JSON.

**Step 3: Rework `openclaw_config.rs` toward upstream structure**

Model the file after upstream as closely as possible:

- provider CRUD against document sections
- `agents.defaults` helpers
- `env` and `tools` preservation helpers
- health/no-op style outcomes where upstream has them

Preserve behavior before optimizing internals.

**Step 4: Re-run the focused config tests**

Run:

```bash
cargo test openclaw_config
```

Expected: PASS.

### Task 5: Align provider-service OpenClaw flows with upstream backend behavior

**Files:**
- Modify: `src-tauri/src/services/provider/live.rs`
- Modify: `src-tauri/src/services/provider/mod.rs`
- Modify: `src-tauri/tests/provider_service.rs`
- Modify: `src-tauri/tests/import_export_sync.rs`
- Modify: `src-tauri/tests/opencode_provider.rs`

**Step 1: Write the failing provider-service parity tests**

Add tests that prove:

- importing OpenClaw live config respects upstream naming/default-model semantics
- writing live config preserves unrelated document sections
- raw fallback only accepts provider-like fragments instead of arbitrary objects
- delete/remove paths use upstream no-op safety semantics
- additive-mode sync keeps OpenCode behavior green while specializing OpenClaw behavior

**Step 2: Run the focused integration tests and watch them fail**

Run:

```bash
cargo test --test provider_service openclaw
cargo test --test import_export_sync openclaw
```

Expected: FAIL because provider-service behavior is still a generalized additive-mode approximation.

**Step 3: Implement the smallest behavior-by-behavior parity fixes**

Update live snapshot read/write/import/delete flows so OpenClaw delegates to upstream-like `openclaw_config` helpers instead of generic JSON blob logic whenever possible.

**Step 4: Re-run the focused integration tests**

Run the same commands from Step 2 and expect PASS.

### Task 6: Verify unsupported and adjacent surfaces stay honest after parity work

**Files:**
- Modify: `src-tauri/src/services/stream_check/service.rs`
- Modify: `src-tauri/src/services/mcp.rs`
- Modify: `src-tauri/tests/stream_check_claude_openai_responses.rs`
- Modify: `src-tauri/tests/proxy_service.rs`

**Step 1: Write or extend failing regression tests**

Prove that after the TUI/backend parity work, OpenClaw still:

- does not fake support for stream-check
- does not participate in unsupported MCP/proxy flows
- reports unsupported surfaces in a way that matches upstream intent

**Step 2: Run the focused tests and watch them fail if parity work regressed them**

Run:

```bash
cargo test --test stream_check_claude_openai_responses openclaw
cargo test --test proxy_service openclaw
```

Expected: PASS or targeted FAILs that reveal parity regressions introduced by Tasks 2-5.

**Step 3: Repair only the regressions revealed by tests**

Do not broaden scope here. Keep unsupported surfaces explicit and boring.

### Task 7: Full verification and cleanup

**Files:**
- Modify only files touched above

**Step 1: Run formatting**

Run:

```bash
cargo fmt
```

**Step 2: Run focused OpenClaw verification**

Run:

```bash
cargo test --locked openclaw
cargo test --test provider_service openclaw
cargo test --test import_export_sync openclaw
```

Expected: PASS.

**Step 3: Run broader safety checks**

Run:

```bash
cargo test --locked provider_service_switch_opencode
cargo test --locked provider_service_delete_openclaw_removes_provider_from_live_and_state
```

Substitute the closest existing focused regression names if these exact filters drift.

**Step 4: Review before claiming completion**

Request one independent code review pass focused on:

- OpenClaw/upstream semantic parity
- additive-mode regressions against OpenCode
- TUI wording and action honesty
