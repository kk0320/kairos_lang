# Kairos Shell

## Start the shell

```powershell
cargo run --bin kairos -- shell examples\assistant_briefing
```

With no path:

- if the current directory is inside a Kairos project, the shell auto-detects it
- otherwise the shell starts in unloaded mode and you can use `:load <path>`

## Startup banner

Kairos prints a branded startup banner and operational summary before showing the prompt:

```text
AI-first programming language shell

version: v1.0.0
mode: project-aware | deterministic
source: project
project: assistant_briefing
entry: demo.assistant_briefing
modules: 3
root: C:/.../examples/assistant_briefing
watch: off

Tips:
:help
:status
:check
:run main
:ir
:modules
:prompt
:reload
:watch
:clear
:quit

kairos>
```

## Shell commands

- `:help`
  Show shell help.
- `:status`
  Show the current mode, source, root, package, entry module, module count, and watch state.
- `:load <path>`
  Load a project, manifest, or `.kai` file into the current session.
- `:check`
  Reload and validate the current target from disk.
- `:ast [selector]`
  Print AST JSON for the current target or a selected module.
- `:ir [selector]`
  Print KIR JSON for the current target or a selected module.
- `:prompt [selector]`
  Print prompt/context markdown for the current target or a selected module.
- `:run [function] [args...]`
  Run the current target with optional function and arguments.
- `:modules`
  List loaded modules and mark entry/focus modules.
- `:reload`
  Reload the current file/project from disk and revalidate it.
- `:watch`
  Start session watch mode.
- `:unwatch`
  Stop session watch mode.
- `:clear`
  Clear the terminal and redraw the banner.
- `:quit`
  Exit the shell.

Selectors for `:ast`, `:ir`, and `:prompt` are exact module names or exact relative paths inside the project.

## `:run` behavior

Shell `:run` is human-oriented. It renders a concise execution report instead of top-level JSON.

Argument parsing follows the same rules as `kairos run`:

- JSON values such as `72`, `true`, `[1,2]`, and `{"ok":true}` are accepted directly
- bare non-JSON text is treated as a string

## Reload and watch

` :reload`:

- reloads the current project or file from disk
- re-runs parsing, project resolution, and semantic validation
- keeps the shell session alive
- prints a success/failure summary

` :watch`:

- watches the current project root recursively, or the current file directory for standalone files
- reloads and revalidates on `.kai` or `kairos.toml` changes
- keeps watch state only for the current shell session
- does not auto-run entry functions by default

` :unwatch` stops the active watcher cleanly.

## Example session

```text
kairos> :status
Kairos shell status
- mode: project-aware | deterministic
- source: project
- project: decision_bundle
- entry: demo.decision_bundle
- modules: 3
- root: C:/.../examples/decision_bundle
- watch: off

kairos> :modules
Loaded modules
- demo.decision_bundle [entry, focus] -> src/main.kai
- demo.decision_bundle.labels -> src/labels.kai
- demo.decision_bundle.scoring -> src/scoring.kai

kairos> :prompt
# Kairos Project Context
...

kairos> :run classify 72
Kairos execution report
- module: demo.decision_bundle
- classify => "MEDIUM"

kairos> :reload
OK: reloaded project `decision_bundle` (entry: demo.decision_bundle, modules: 3, warnings: 0)

kairos> :watch
Watch mode enabled.
```

## Current limitations

- the shell is line-oriented rather than full-screen
- shell output is human-oriented and does not replace stable top-level JSON commands
- watch notifications may appear between prompts while you are typing
- persisted shell history is not implemented yet
