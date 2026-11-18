# Builtins & Configuration

`lsh` provides built-in commands alongside an extensible configuration system (`config.lsh`).

---

## Table of Contents

- [Builtin Commands](#builtin-commands)
  - [`alias`](#alias)
  - [`bg`](#bg)
  - [`cd`](#cd)
  - [`complete`](#complete)
  - [`disown`](#disown)
  - [`exec`](#exec)
  - [`exit`](#exit)
  - [`export`](#export)
  - [`fg`](#fg)
  - [`fmt-cwd`](#fmt-cwd)
  - [`jobs`](#jobs)
  - [`kill`](#kill)
  - [`plugin`](#plugin)
  - [`source`](#source)
  - [`wait`](#wait)
- [Configuration (`config.lsh`)](#configuration-configlsh)
  - [Options (`$options`)](#options-options)
  - [Colours (`$palette`)](#colours-palette)
  - [Prompt Customisation (`$prompt`)](#prompt-customisation-prompt)
  - [Lifecycle Hooks (`$hooks`)](#lifecycle-hooks-hooks)
  - [JSON (`$json`)](#json-json)
  - [Plugins & Modules (`import`)](#plugins--modules-import)

---

## Builtin Commands

### `alias`
Define and inspect command aliases:

```lsh
alias set ls "eza --icons"
alias list
```

### `bg`
Resume a stopped job in the background:

```lsh
bg %1
```

### `cd`
Change the current working directory (normalises logical paths and expands `~`):

```lsh
cd ~/projects
cd /var/tmp
```

`cd` automatically synchronises `$PWD` and `$OLDPWD` in both the shell scope and the operating system process environment, and triggers `chpwd` hooks.

### `complete`
Configure custom tab completion rules (`set`, `list`, `remove`):

```lsh
complete set mytool start stop:Stop status --ext json
complete set mycd --dirs
complete set mygit --exec "git branch"
complete list
complete remove mytool
```

### `disown`
Remove jobs from the shell job table (`-a` removes all jobs):

```lsh
disown %1
disown -a
```

### `exec`
Execute a script file in a fresh, isolated evaluator:

```lsh
exec ./script.lsh
```

### `exit`
Exit the shell session with an optional exit code:

```lsh
exit 0
```

### `export`
Set, export, or list environment variables:

```lsh
export KEY="value"
export
```

### `fg`
Resume a background or stopped job in the foreground:

```lsh
fg %1
```

### `fmt-cwd`
Format the current directory for prompt display, substituting `$HOME` with `~`:

```lsh
fmt-cwd
```

### `jobs`
List background and stopped jobs (supports `-l`, `-p`, `-r`, `-s`):

```lsh
jobs
jobs -l
```

### `kill`
Send a signal to processes or job specs:

```lsh
kill %1
kill -9 12345
kill -TERM %1
kill -l
```

### `plugin`
Manage loaded plugins (plugins are loaded using `import("my-plugin")`):

```lsh
plugin list
plugin unload my-plugin
plugin info my-plugin
```

### `source`
Execute a script within the current shell evaluator (persisting variables and functions):

```lsh
source ~/.config/lsh/config.lsh
```

### `wait`
Wait for background jobs to complete:

```lsh
wait
wait %1
```

---

## Configuration (`config.lsh`)

At startup, `lsh` looks for `config.lsh` in:
1. `$LSH_CONFIG_DIR/config.lsh` (if `LSH_CONFIG_DIR` is set)
2. `<platform config dir>/lsh/config.lsh` (e.g. `~/.config/lsh/config.lsh` on Linux)
3. `./config/config.lsh` if no platform config directory is available

---

### Options (`$options`)

Global shell options configured via the `$options` object:

```lsh
# Enable Nerd Font icons in the autocomplete dropdown:
$options.nerd_icons = true

# Tab stop width (number of spaces):
$options.tab_stop = 4

# Keybindings: "default", "emacs", or "vi"
$options.edit_mode = "default"

# Cursor shape: "bar", "steady_bar", "block", "steady_block", "underline", or "steady_underline"
$options.cursor_shape = "bar"

# Completion UI: "menu" (fuzzy dropdown), "list", or "circular"
$options.completion_type = "menu"
```

---

### Colours (`$palette`)

The `$palette` object provides ANSI escape codes:

```lsh
# Basic colours:
$palette.black
$palette.red
$palette.green
$palette.yellow
$palette.blue
$palette.magenta
$palette.cyan
$palette.white
$palette.default

# Bright variants:
$palette.bright_red
$palette.bright_green
$palette.bright_blue
$palette.bright_magenta

# Styles:
$palette.bold
$palette.dim
$palette.italic
$palette.underline
$palette.reset
```

---

### Prompt Customisation (`$prompt`)

Configure segments rendered in the interactive prompt:

```lsh
$prompt.segments = [
    "\n",
    () => { return $palette.bright_blue + fmt_cwd() + $palette.reset },
    "\n",
    $palette.bright_magenta + "❯ " + $palette.reset
]

$prompt.separator = " "
```

---

### Lifecycle Hooks (`$hooks`)

Register event handlers for shell lifecycle events:

- `"pre_exec"` — Before executing a command (receives command string)
- `"post_exec"` — After command finishes (receives command string and exit status)
- `"pre_prompt"` — Before rendering prompt
- `"chpwd"` — When working directory changes (receives old and new path)

```lsh
# Notify when directory changes
$hooks.add("chpwd", (old_dir, new_dir) => {
    echo "Switched to: " + $new_dir
})

# List registered hooks (returns an object mapping event name -> count)
let registered = $hooks.list()

# Clear all hooks for an event
$hooks.clear("chpwd")
```

---

### JSON (`$json`)

Parse and serialise JSON values cleanly between strings, command results, and `lsh` objects:

```lsh
# Parse a JSON string (or a $(...) command result object):
let data = $json.parse('{"name": "lsh", "tags": ["shell", "rust"]}')
echo $data.name          # "lsh"
echo $data.tags[0]       # "shell"

# Or call .json() directly on strings or $(cmd) results:
let user = $(printf '{"id": 1, "active": true}').json()

# Serialise to compact or pretty-printed JSON:
let compact = $json.stringify($data)
let pretty = $json.stringify($data, true) # 2-space indentation (or pass a custom number of spaces)
```

---

### Plugins & Modules (`import`)

Load external modules or prompt plugins using `import(...)`:

```lsh
let git_prompt = import("git-prompt")

$prompt.segments = [
    fmt_cwd(),
    $git_prompt.segment,
    "\n❯ "
]
```
