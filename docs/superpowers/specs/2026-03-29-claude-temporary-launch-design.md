# Claude Temporary Launch From TUI

## Summary

Add a temporary launch action to the interactive TUI for Claude providers.

From the provider list or provider detail page, the user can press a dedicated key to start a one-off Claude session with the selected provider. This action does not switch the current global provider and does not write the selected provider into Claude live config as the new default.

`cc-switch` acts as a launcher. If the launch precheck passes, `cc-switch` restores the terminal and hands control to `claude` in the current terminal session. `cc-switch` does not come back after that.

## User Goal

The user keeps normal `claude` usage unchanged. The default provider still comes from the current live config.

When the user wants to try another provider for one terminal session, they open `cc-switch`, select a provider, trigger temporary launch, and go straight into Claude with that provider's temporary settings.

## Scope

In scope:

- Claude app in the ratatui TUI
- Provider list page
- Provider detail page
- One-off launch in the current terminal
- Launcher-level precheck and error reporting inside TUI

Out of scope:

- Codex, Gemini, OpenCode, OpenClaw
- Preferred terminal settings
- Opening a new external terminal window
- Returning to TUI after Claude exits
- Making the launched provider the current global provider

## UX

### Entry points

The action is available only when `app_type == Claude`.

It appears in two places:

- provider list
- provider detail

The action gets its own key hint in the inline key bar. The recommended key is `o`.

Copy should make the behavior explicit. This is a temporary launch, not a switch.

Suggested label:

- Chinese: `临时启动`
- English: `Launch Temp`

### Success path

1. User highlights a Claude provider.
2. User presses the temporary launch key.
3. `cc-switch` runs launch prechecks.
4. If prechecks pass, TUI restores the terminal and transfers control to `claude` in the current terminal.
5. `cc-switch` does not return.

### Failure path

If launcher-level prechecks fail, the user stays inside TUI and sees an error toast.

This covers failures such as:

- `claude` command not found
- temporary settings file cannot be created
- provider data cannot be converted into launch settings

This does not try to catch or reinterpret runtime failures after Claude has already started.

If Claude starts and later reports auth or API errors, those belong to the Claude session, not to `cc-switch`.

## Behavior

### Global state

Temporary launch does not call provider switch logic.

It must not:

- update the current provider in `cc-switch`
- overwrite Claude live config as the selected provider
- trigger provider switch tips or first-use overwrite guards

### Provider-derived settings

The launched Claude session uses a temporary settings file derived from the selected provider.

The first version should stay close to upstream behavior:

- read Claude provider `settings_config`
- extract the `env` object
- write a temporary Claude settings JSON file that contains those env values
- launch `claude --settings <temp-file>`

The temporary file should be removed automatically when possible. If cleanup cannot happen in every path, stale temp files are acceptable for the first version as long as names are unique and contents are correct.

### Process handoff

The happy path should replace the `cc-switch` process with `claude`, or achieve the same user-visible effect by launching Claude and exiting immediately after a successful handoff.

The design goal is simple:

- before handoff, `cc-switch` still owns TUI and can show launch errors
- after handoff, the terminal belongs to Claude and `cc-switch` is gone

Using direct `exec` on Unix is preferred if it keeps the implementation small and reliable.

## Implementation Shape

### TUI action layer

Add a new provider action for Claude temporary launch.

Affected areas:

- key handling in provider list
- key handling in provider detail
- key bar rendering
- localized key labels and toasts

This action should be available only for Claude.

### Runtime action layer

Add a runtime action that:

1. resolves the selected provider
2. runs launch prechecks
3. restores the terminal
4. hands off to Claude

This should be separate from normal provider switching. Do not reuse `ProviderSwitch` or any code path that mutates the current provider.

### Launch helper

Add a focused helper for Claude temporary launch. Keep it outside large TUI files if that keeps file size under control.

Recommended responsibilities:

- build launch settings from provider data
- write temporary settings file
- resolve `claude` executable
- start or exec Claude with `--settings`

The helper should return structured errors up to the TUI action so the failure case can stay in the TUI.

## Error Handling

Pre-handoff errors stay in `cc-switch`:

- provider not found
- malformed provider settings
- temp file write failure
- `claude` executable not found
- process spawn or exec failure

Post-handoff errors belong to Claude:

- invalid API key
- bad base URL
- upstream service failures
- Claude startup logic errors after process transfer

This keeps the boundary simple and avoids `cc-switch` trying to supervise another CLI.

## Testing

Add focused tests for:

- provider list key dispatch for Claude temporary launch
- provider detail key dispatch for Claude temporary launch
- key bar text for Claude pages
- action hidden for non-Claude apps
- launch helper error when `claude` is missing
- launch helper builds a temporary settings payload from provider env
- runtime action does not mutate current provider on failure

Prefer unit tests around helper boundaries and existing TUI action tests over broad end-to-end terminal process tests.

## Risks

- Terminal restore must happen before handoff. If ordering is wrong, the user can end up inside Claude with broken terminal state.
- Direct `exec` behavior differs by platform. The first implementation may need a small platform split.
- Temp file cleanup is easy to over-engineer. Keep the first version simple.

## Recommendation

Ship this as a Claude-only TUI feature with one key, one launch path, and no settings UI.

That version matches the user need, keeps the boundary small, and avoids the extra complexity in upstream's external terminal launcher flow.
