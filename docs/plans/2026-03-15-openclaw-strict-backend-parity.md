# OpenClaw Strict Backend Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make OpenClaw backend/config behavior match upstream semantics as closely as possible, then tighten the TUI so it exposes the same truth with keyboard-first interaction instead of web-style UI imitation.

**Architecture:** Treat `src-tauri/src/openclaw_config.rs` as the parity center of gravity. First replace the current provider-centric JSON blob writer with upstream-like JSON5 document helpers, typed section APIs, write outcomes, health scanning, and no-op/conflict semantics. Then rewire provider services to delegate to those helpers. Only after backend parity is proven by tests do we continue polishing the TUI, and TUI changes must optimize for terminal best practices rather than matching upstream button-for-button.

**Tech Stack:** Rust, serde/serde_json, `json5`, `json_five::rt`, ratatui, Rust unit tests, Rust integration tests.

**Execution Note:** Plan steps include commit checkpoints for change grouping, but do not execute any `git commit` step unless the user explicitly asks for a commit in this conversation.

---

## Strict Acceptance Gates

Work is not accepted unless all gates below are satisfied with fresh evidence.

### Gate A: Backend semantic parity

- `read_openclaw_config()` parses real JSON5, not a homegrown compatibility subset.
- Writes preserve unrelated root sections and existing JSON5 document structure as far as the upstream round-trip writer does.
- Changed writes return a typed write outcome with `backup_path` and `warnings`.
- Unchanged writes do not rewrite the file and do not create a backup.
- On-disk conflict detection rejects writes when the source changed after load.
- Missing-provider removal is a true no-op.
- `agents.defaults.model`, `agents.defaults.models`, full `agents.defaults`, `env`, and `tools` all have typed read/write helpers.
- Health scan surfaces upstream-style warnings for parse errors and malformed `env` / `tools` values.

### Gate B: Service parity

- OpenClaw provider-service flows delegate to the strict backend helpers instead of generic additive-mode JSON mutation.
- Import/export/live snapshot flows preserve unrelated OpenClaw document sections.
- Default-model mutations use live OpenClaw provider order only.

### Gate C: TUI truthfulness

- TUI help, actions, and detail state reflect actual OpenClaw semantics.
- TUI never invents unsupported OpenClaw capability.
- TUI interaction can diverge from upstream web UX, but only when terminal UX is clearly better and backend semantics stay identical.

### Gate D: Verification bundle

Before claiming completion of any phase, run the exact focused commands listed in that phase and confirm expected pass/fail behavior. Before claiming the full plan is complete, run at least:

```bash
cd src-tauri && cargo test openclaw_config -- --nocapture
cd src-tauri && cargo test --test provider_service openclaw -- --nocapture
cd src-tauri && cargo test --test import_export_sync openclaw -- --nocapture
cd src-tauri && cargo test openclaw_set_default_model -- --nocapture
cd src-tauri && cargo test extract_primary_model_id_openclaw -- --nocapture
```

If any command fails, the phase is not complete.

---

## Current Status (2026-03-15)

This plan is no longer just a proposal. Most of the work listed below has been implemented in the `openclaw-dev` worktree and verified with focused Rust tests.

### What is done

- Task 1 is done. `src-tauri/src/openclaw_config.rs` now uses real JSON5 parsing, a round-trip document writer, typed write outcomes, backup creation, unchanged-write no-op handling, and conflict detection.
- Task 2 is done. The OpenClaw config layer now has typed helpers for `agents.defaults.model`, `agents.defaults.models`, full `agents.defaults`, `env`, and `tools`, plus health warnings for malformed values.
- Task 3 is done. OpenClaw provider-service and live snapshot flows now route through strict config helpers instead of generic additive-mode JSON rewriting.
- Task 4 is done. TUI runtime state now treats providers referenced only through `agents.defaults.model.fallbacks` as real default-model references for remove-from-config protection, while still reserving `is_default_model` for the actual primary default provider and leaving full delete to backend semantics.
- Task 5 focused verification passed. Existing help/action/detail behavior for OpenClaw matched the stricter semantics after Task 4, so no extra corrective pass was required before verification.
- Task 6 focused verification passed. The OpenClaw form path already had broad coverage for the structured keyboard-first model workflow added earlier in this branch.
- Follow-up parity cleanup is now done as well. The worktree now includes settings-driven backup retention, upstream-compatible `ProviderMeta` serialization, OpenClaw-skipping MCP migration, stricter OpenClaw import/live-sync semantics, and shared-state test serialization across OpenClaw unit/runtime tests.
- A final upstream-tightening pass also removed the last known local OpenClaw import deviations: model-less providers are now skipped unconditionally during import, and imported provider display names now use only the first model name or the provider id, without falling back to the top-level provider name.
- The remaining high-priority common-config backfill drift is now fixed. When switching away from Claude/Codex/Gemini providers, the saved snapshot strips shared common-config fields only if that provider explicitly enables common config or legacy inference proves it is using the shared snippet.
- The medium-priority durability gap in `update()` is also fixed for non-additive apps. Updating the current Claude/Codex/Gemini provider now refreshes the saved snapshot after the live write, so legacy `meta = None` providers are normalized durably instead of only behaving correctly in-memory during the write path.
- A follow-up review found one more durability hole in the first hardening pass: `switch-away` backfill stripped common-config values for legacy providers without simultaneously normalizing `meta.apply_common_config = Some(true)`. That is now fixed too, so a `switch away -> switch back` round-trip keeps applying shared common config instead of silently losing it after the first backfill.
- A second follow-up review found a deeper parity mismatch between legacy subset detection and actual stripping behavior. That is now fixed by switching the local removal logic to upstream-like deep removal semantics: JSON arrays remove only the shared subset, and Codex TOML tables remove only the shared nested items instead of dropping entire root tables.
- A final follow-up review found one more Codex-only drift in the current-provider refresh path. Updating a current Codex provider with `applyCommonConfig = false` could still infer and persist a shared global snippet from provider-owned TOML fields. That is now fixed too, so explicit common-config opt-out remains durable across update/refresh flows.
- The first TUI truthfulness gap from review is now closed too. OpenClaw live-only providers are no longer invisible in the TUI: provider-row loading now synthesizes row state from live-only providers without importing them into the saved snapshot, and existing snapshot rows prefer live OpenClaw settings/name values so the list, detail, and edit entry reflect current `openclaw.json` truth instead of stale snapshot data.
- The second important TUI parity gap from review is now closed as well. OpenClaw provider-list `d` no longer blocks deletion just because the provider owns the current default or fallback model reference; it now opens the normal confirm flow and leaves the final semantics to the backend, matching the upstream-like additive delete behavior already enforced in service tests.
- A follow-up review then found two more TUI truthfulness drifts in the same area, and both are now fixed. OpenClaw `x` no longer becomes a no-op when the selected provider is already the primary default, so the runtime action can still rebuild fallback order from live config. OpenClaw `e` also no longer blocks `saved-only` providers; if a provider exists in the saved snapshot, the TUI now allows editing it and writing it back into live config, which matches the additive backend update path.
- The remaining mixed-language OpenClaw detail copy is now fixed too. Status/model labels and OpenClaw row-state labels now go through `src-tauri/src/cli/i18n.rs`, and Chinese detail rendering is covered by focused TUI/i18n tests instead of hard-coded English strings in the renderer.
- A final backend hardening pass is now done for the serializer fallback edge case too. Removing the last OpenClaw provider no longer rewrites the whole `models` section through the JSON fallback path; it now removes the final nested provider entry directly in the round-trip AST, preserving JSON5-style `mode`, preserved comments, and unrelated source text while still leaving `providers: {}`.
- The final OpenClaw UI evidence gap is now closed as well. Focused TUI tests cover every non-default row-state label in Chinese detail rendering plus Chinese key-bar labels for both the list and detail views, so the current i18n wiring is verified at render level rather than only through string helpers.

### Important implementation notes

- OpenClaw live snapshot rollback now restores the original source text verbatim instead of rewriting the file through generic JSON serialization. This avoids dropping JSON5 comments and formatting during rollback.
- OpenClaw remove-from-config protection now covers providers referenced only through fallback model refs, not just the provider referenced by `primary`, while full delete intentionally remains allowed and defers the final semantics to the backend.
- In TUI state, `default_model_id` may come from either `primary` or `fallbacks`, but `is_default_model` still means the provider owning the actual primary default ref. This keeps remove blocking truthful without collapsing the distinction between primary and fallback references.
- OpenClaw default import now prefers typed providers, imports incrementally, skips malformed/model-less entries unconditionally like upstream, and keeps unrelated JSON5 document sections untouched.
- OpenClaw tests that inspect live config text now parse JSON5 instead of assuming strict JSON, which matches the actual OpenClaw file contract.
- The import/export source-text preservation fixture was updated so it still proves zero-rewrite behavior without relying on the old non-upstream model-less import special case.

### Focused verification evidence already collected

The following commands were run successfully in `src-tauri`:

```bash
cargo test openclaw_default_model_ids_by_provider -- --nocapture
cargo test openclaw_remove_from_config_rejects_fallback_only_provider_even_without_ui_guard -- --nocapture
cargo test openclaw_providers_s_key_blocks_removing_fallback_only_default_provider -- --nocapture
cargo test openclaw_provider_detail_s_key_blocks_removing_fallback_only_default_provider -- --nocapture
cargo test openclaw_providers_x_key_promotes_fallback_only_provider_even_when_model_matches_primary -- --nocapture
cargo test openclaw_provider_detail_x_key_promotes_fallback_only_provider_even_when_model_matches_primary -- --nocapture
cargo test openclaw_set_default_model -- --nocapture
cargo test extract_primary_model_id_openclaw -- --nocapture
cargo test cli::tui::app::tests::tests::openclaw -- --nocapture
cargo test cli::tui::ui::tests::openclaw -- --nocapture
cargo test provider_add_form_openclaw -- --nocapture
cargo test provider_edit_form_openclaw -- --nocapture
cargo test openclaw_config -- --nocapture
cargo test --test provider_service openclaw -- --nocapture
cargo test --test import_export_sync openclaw -- --nocapture
cargo test provider_meta_serializes_upstream_common_config_key_and_accepts_legacy_alias -- --nocapture
cargo test backup_cleanup_uses_settings_retain_count -- --nocapture
cargo test migrate_mcp_to_unified_keeps_openclaw_legacy_servers_unmigrated -- --nocapture
cargo test provider_service_import_default_openclaw -- --nocapture
cargo test openclaw_add_skips_non_provider_like_object_when_syncing_live_config -- --nocapture
cargo test openclaw -- --nocapture
cargo test provider_service_import_default_openclaw_skips_modeless_provider_even_if_default_references_it -- --nocapture
cargo test provider_service_import_default_openclaw_uses_provider_id_when_primary_model_has_no_name -- --nocapture
cargo test provider_service_import_default_openclaw_ignores_later_model_name_when_first_model_has_no_name -- --nocapture
cargo test import_openclaw_live_config_preserves_unrelated_root_sections_and_source_text -- --nocapture
cargo test import_openclaw_live_config_skips_modeless_default_provider_without_rewriting_source_text -- --nocapture
cargo test openclaw_providers_d_key_allows_deleting_provider_referenced_by_default_model -- --nocapture
cargo test reapplies_primary_default -- --nocapture
cargo test saved_only_provider -- --nocapture
cargo test openclaw_provider_status_copy_is_fully_localized_in_chinese -- --nocapture
cargo test openclaw_provider_detail_localizes_status_copy_in_chinese -- --nocapture
cargo test remove_last_provider_preserves_models_section_source_text -- --nocapture
cargo test openclaw_provider_detail_localizes_non_default_status_variants_in_chinese -- --nocapture
cargo test openclaw_provider_list_key_bar_localizes_actions_in_chinese -- --nocapture
cargo test openclaw_provider_detail_key_bar_localizes_actions_in_chinese -- --nocapture
```

### Verification note for Task 6

- The original Task 6 filter in this document, `cargo test cli::tui::form::tests::openclaw -- --nocapture`, currently matches zero tests in this codebase.
- The actual focused verification used `cargo test provider_add_form_openclaw -- --nocapture` and `cargo test provider_edit_form_openclaw -- --nocapture`, which exercised the existing OpenClaw form tests directly.

### Broader suite note

- The broader filter run is now clean: `cargo test openclaw -- --nocapture` passes.
- The earlier order-dependent failures were traced to shared process state (`HOME` / settings store) across OpenClaw tests and addressed by serializing the OpenClaw config test module with the same `serial_test` mechanism already used in runtime-action tests.
- The final optional import-coverage gaps are now closed with dedicated regressions: one test combines model-less-provider skipping with zero-rewrite source preservation in the same JSON5 document, and another proves that a later named model does not override the first-model-only display-name rule.
- The common-config backfill/update hardening pass now has focused regression coverage too:
  - `codex_switch_away_preserves_provider_owned_fields_when_common_config_is_disabled`
  - `codex_update_normalizes_legacy_common_config_into_explicit_meta`
  - `codex_switch_roundtrip_preserves_legacy_common_config_usage_after_backfill`
  - `claude_switch_strips_common_array_items_but_preserves_provider_specific_ones`
  - `codex_switch_away_preserves_provider_specific_fields_inside_partially_shared_root_table`
  - `codex_update_with_common_config_disabled_does_not_extract_global_snippet`
  - `load_providers_openclaw_imports_live_only_provider_into_snapshot_rows`
  - `load_providers_openclaw_prefers_live_values_for_existing_snapshot_provider`
- Fresh verification after that hardening pass succeeded with:

```bash
cargo test codex_switch_away_preserves_provider_owned_fields_when_common_config_is_disabled -- --nocapture
cargo test codex_update_normalizes_legacy_common_config_into_explicit_meta -- --nocapture
cargo test codex_switch_roundtrip_preserves_legacy_common_config_usage_after_backfill -- --nocapture
cargo test claude_switch_strips_common_array_items_but_preserves_provider_specific_ones -- --nocapture
cargo test codex_switch_away_preserves_provider_specific_fields_inside_partially_shared_root_table -- --nocapture
cargo test codex_update_with_common_config_disabled_does_not_extract_global_snippet -- --nocapture
cargo test common_config -- --nocapture
cargo test openclaw -- --nocapture
```

- Latest bundle verification was re-checked from the saved command output at `/Users/saladday/.local/share/opencode/tool-output/tool_cf1cf68ca001WIsc2u6UmHWX6P`:
  - `cargo test common_config -- --nocapture` -> `22 passed; 0 failed`
  - `cargo test openclaw -- --nocapture` -> `61 passed; 0 failed`
- Additional fresh TUI-focused verification after the OpenClaw visibility/truthfulness fix succeeded with:

```bash
cargo test load_providers_openclaw_imports_live_only_provider_into_snapshot_rows -- --nocapture
cargo test load_providers_openclaw_prefers_live_values_for_existing_snapshot_provider -- --nocapture
cargo test openclaw -- --nocapture
```

- Latest fresh `cargo test openclaw -- --nocapture` now reports `76 passed; 0 failed` after the delete-parity, `x` reapply, saved-only edit truthfulness, localization, and remove-last-provider source-preservation regressions were added.

### Closure note

- The strict backend-parity and TUI-truthfulness gates listed in this document now have fresh verification evidence.
- The remove-last-provider serializer fallback gap is covered by source-text-preservation regression tests, and the OpenClaw UI/i18n render surface now has direct regression coverage for the previously missing status/key-bar cases.
- Re-reading the touched areas against `.upstream` did not reveal a remaining backend-semantic drift that blocks submission; the remaining local differences are the intentional local hardening needed around the empty-object serializer bug and the TUI-only discoverability/i18n layer.
- As of the latest verification bundle in this worktree, there is no remaining blocker in this plan before submission.

---

### Task 1: Port upstream write outcome, health, and document primitives

**Files:**
- Modify: `src-tauri/src/openclaw_config.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/openclaw_config.rs`

**Step 1: Write the failing tests**

Add unit tests that prove:

- parse warnings are returned instead of hard failure when config text is invalid JSON5
- changed writes return a non-empty `backup_path`
- unchanged writes return `backup_path = None`
- changing one root section preserves unrelated sections and comments/ordering well enough to match upstream behavior
- write attempts fail when the on-disk source changes after load

**Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test openclaw_config::tests:: -- --nocapture
```

Expected: FAIL because current code has no `OpenClawWriteOutcome`, no document round-trip writer, and no health scan API.

**Step 3: Write minimal implementation**

Port the upstream-style infrastructure into `src-tauri/src/openclaw_config.rs`:

- `OpenClawHealthWarning`
- `OpenClawWriteOutcome`
- JSON5 parsing via `json5::from_str`
- `OpenClawConfigDocument` with round-trip `json_five::rt` loading/saving
- single-writer lock
- atomic save, backup creation, unchanged-write short circuit, conflict detection
- root-section write helper

Do not yet rewire all callers. First make the primitives and tests green.

**Step 4: Run tests to verify they pass**

Run the same command from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/openclaw_config.rs src-tauri/src/lib.rs
git commit -m "refactor(openclaw): add strict document write primitives"
```

### Task 2: Port typed section helpers for `agents.defaults`, `env`, and `tools`

**Files:**
- Modify: `src-tauri/src/openclaw_config.rs`
- Modify: `src-tauri/src/provider.rs`
- Test: `src-tauri/src/openclaw_config.rs`

**Step 1: Write the failing tests**

Add unit tests that prove:

- `get_default_model()` / `set_default_model()` preserve extra fields
- `get_default_models_catalog()` / `set_default_models_catalog()` read and write `agents.defaults.models`
- `get_agents_defaults()` / `set_agents_defaults()` round-trip the full `agents.defaults` subtree
- `get_env_config()` / `set_env_config()` and `get_tools_config()` / `set_tools_config()` work through typed helpers
- malformed `env.vars`, `env.shellEnv`, and invalid `tools.profile` generate health warnings

**Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test openclaw_config::tests:: -- --nocapture
```

Expected: FAIL because current code only covers `agents.defaults.model` and lacks full section helpers.

**Step 3: Write minimal implementation**

Port upstream-like typed structs and helpers for:

- `OpenClawModelCatalogEntry`
- `OpenClawAgentsDefaults`
- `OpenClawEnvConfig`
- `OpenClawToolsConfig`
- `scan_openclaw_config_health()` and value-based warning helpers

Keep names and behavior as close to upstream as practical.

**Step 4: Run tests to verify they pass**

Run the same command from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/openclaw_config.rs src-tauri/src/provider.rs
git commit -m "feat(openclaw): add typed section helpers"
```

### Task 3: Rewire OpenClaw provider-service and live snapshot flows to strict helpers

**Files:**
- Modify: `src-tauri/src/services/provider/mod.rs`
- Modify: `src-tauri/src/services/provider/live.rs`
- Modify: `src-tauri/src/openclaw_config.rs`
- Test: `src-tauri/tests/provider_service.rs`
- Test: `src-tauri/tests/import_export_sync.rs`

**Step 1: Write the failing tests**

Add integration tests that prove:

- importing OpenClaw live config preserves unrelated root sections
- removing a live provider is a no-op when missing
- deleting/removing the provider containing the default model is rejected in both UI and backend paths
- live snapshot capture/restore preserves document sections outside `models.providers`

**Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test --test provider_service openclaw -- --nocapture
cd src-tauri && cargo test --test import_export_sync openclaw -- --nocapture
```

Expected: FAIL because provider service still assumes the simplified config writer.

**Step 3: Write minimal implementation**

Update service and live snapshot code so OpenClaw paths delegate to strict `openclaw_config` helpers instead of generic JSON rewrites.

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/services/provider/mod.rs src-tauri/src/services/provider/live.rs src-tauri/tests/provider_service.rs src-tauri/tests/import_export_sync.rs
git commit -m "refactor(openclaw): route services through strict config helpers"
```

### Task 4: Tighten default-model runtime semantics around strict backend truth

**Files:**
- Modify: `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- Modify: `src-tauri/src/cli/tui/data.rs`
- Test: `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- Test: `src-tauri/src/cli/tui/data.rs`

**Step 1: Write the failing tests**

Add or extend tests that prove:

- `x` always derives default selection from live config order
- stale snapshot data never leaks back into `primary_model_id`
- unchanged strict backend writes do not alter TUI-visible state spuriously

**Step 2: Run tests to verify they fail if parity regressed**

Run:

```bash
cd src-tauri && cargo test openclaw_set_default_model -- --nocapture
cd src-tauri && cargo test extract_primary_model_id_openclaw -- --nocapture
```

Expected: PASS only when backend truth and TUI state derivation remain aligned.

**Step 3: Write minimal implementation**

Adjust TUI runtime/data code only where strict backend semantics require it. Do not add web-style interaction just to imitate upstream.

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/cli/tui/runtime_actions/providers.rs src-tauri/src/cli/tui/data.rs
git commit -m "fix(openclaw): keep TUI default state aligned with live config"
```

### Task 5: Make the TUI help and list/detail interaction honest and terminal-native

**Files:**
- Modify: `src-tauri/src/cli/i18n.rs`
- Modify: `src-tauri/src/cli/tui/ui/providers.rs`
- Modify: `src-tauri/src/cli/tui/app/content_entities.rs`
- Test: `src-tauri/src/cli/tui/ui/tests.rs`
- Test: `src-tauri/src/cli/tui/app/tests.rs`

**Step 1: Write the failing tests**

Add tests that prove:

- OpenClaw help text advertises `add/remove` and `set default`, not generic `switch`
- unsupported shortcuts stay hidden
- any confirmation or blocking behavior follows TUI best practices and backend truth, not web parity for its own sake

**Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test cli::tui::app::tests::tests::openclaw -- --nocapture
cd src-tauri && cargo test cli::tui::ui::tests::openclaw -- --nocapture
```

Expected: FAIL where help text or action exposure still uses generic provider wording.

**Step 3: Write minimal implementation**

Bring help text and interaction copy in line with real OpenClaw semantics. Prefer concise TUI-native phrasing over upstream web labels.

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/cli/i18n.rs src-tauri/src/cli/tui/ui/providers.rs src-tauri/src/cli/tui/app/content_entities.rs
git commit -m "fix(openclaw): align TUI help with additive semantics"
```

### Task 6: Upgrade OpenClaw form editing for terminal-native multi-model workflows

**Files:**
- Modify: `src-tauri/src/cli/tui/form.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state_loading.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_json.rs`
- Modify: `src-tauri/src/cli/tui/ui/forms/provider.rs`
- Modify: `src-tauri/src/cli/tui/app/form_handlers/provider.rs`
- Test: `src-tauri/src/cli/tui/form/tests.rs`

**Step 1: Write the failing tests**

Add tests that prove:

- OpenClaw model editing is structured enough for keyboard-first use
- primary/fallback ordering is explicit
- typed edits preserve unknown fields when saved back
- TUI still allows raw JSON escape hatches for advanced values without making them the primary path

**Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test cli::tui::form::tests::openclaw -- --nocapture
```

Expected: FAIL where the form still relies too heavily on raw JSON editing.

**Step 3: Write minimal implementation**

Improve the TUI editor toward a structured model list workflow. Do not attempt to replicate upstream web layout or every preset surface.

**Step 4: Run tests to verify they pass**

Run the same command from Step 2 and expect PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/cli/tui/form.rs src-tauri/src/cli/tui/form/provider_state.rs src-tauri/src/cli/tui/form/provider_state_loading.rs src-tauri/src/cli/tui/form/provider_json.rs src-tauri/src/cli/tui/ui/forms/provider.rs src-tauri/src/cli/tui/app/form_handlers/provider.rs src-tauri/src/cli/tui/form/tests.rs
git commit -m "feat(openclaw): improve terminal-native model editing"
```

### Task 7: Run final strict verification and gap audit

**Files:**
- Modify: `docs/plans/2026-03-15-openclaw-strict-backend-parity.md`

**Step 1: Run the full focused verification bundle**

Run:

```bash
cd src-tauri && cargo test openclaw_config -- --nocapture
cd src-tauri && cargo test --test provider_service openclaw -- --nocapture
cd src-tauri && cargo test --test import_export_sync openclaw -- --nocapture
cd src-tauri && cargo test openclaw_set_default_model -- --nocapture
cd src-tauri && cargo test extract_primary_model_id_openclaw -- --nocapture
cd src-tauri && cargo test cli::tui::ui::tests::openclaw -- --nocapture
cd src-tauri && cargo test cli::tui::app::tests::tests::openclaw -- --nocapture
cd src-tauri && cargo test provider_add_form_openclaw -- --nocapture
cd src-tauri && cargo test provider_edit_form_openclaw -- --nocapture
```

Expected: all PASS.

**Step 2: Re-read acceptance gates one by one**

Check every Gate A-D requirement against code and test evidence. Record any remaining gap explicitly in this plan file instead of hand-waving it away.

**Step 3: Commit**

```bash
git add docs/plans/2026-03-15-openclaw-strict-backend-parity.md
git commit -m "docs(openclaw): record strict parity verification"
```
