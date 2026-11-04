# Builtins & Configuration

`lsh` provides built-in commands alongside an extensible configuration system (`config.lsh`).

---

## Table of Contents

- [Builtin Commands](#builtin-commands)
  - [`alias`](#alias)
  - [`cd`](#cd)
  - [`complete`](#complete)
  - [`exec`](#exec)
  - [`exit`](#exit)
  - [`export`](#export)
  - [`fmt-cwd`](#fmt-cwd)
  - [`plugin`](#plugin)
  - [`source`](#source)
- [Configuration (`config.lsh`)](#configuration-configlsh)
  - [Options (`$options`)](#options-options)
  - [Colours (`$palette`)](#colours-palette)
  - [Prompt Customisation (`$prompt`)](#prompt-customisation-prompt)
  - [Lifecycle Hooks (`$hooks`)](#lifecycle-hooks-hooks)
  - [Plugins & Modules (`import`)](#plugins--modules-import)

---

## Builtin Commands

### `alias`
Define and inspect command aliases:

```lsh
alias set ls "eza --icons"
alias list
```

### `cd`
Change the current working directory:

```lsh
cd ~/projects
cd -P /var/tmp   # Physical path (resolves symlinks)
cd -L /var/tmp   # Logical path (preserves symlinks)
```

`cd` automatically synchronises `$PWD` and `$OLDPWD` in both the shell scope and the operating system process environment.

### `complete`
Configure custom tab completion rules:

```lsh
complete set cargo --flags
complete list
complete remove cargo
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

### `fmt-cwd`
Format the current directory for prompt display, substituting `$HOME` with `~`:

```lsh
fmt-cwd
```

### `plugin`
Manage plugins:

```lsh
plugin list
plugin load ./plugins/my-plugin
plugin unload my-plugin
plugin info my-plugin
```

### `source`
Execute a script within the current shell evaluator (persisting variables and functions):

```lsh
source ~/.config/lsh/config.lsh
```

---

## Configuration (`config.lsh`)

At startup, `lsh` looks for `config.lsh` in:
1. `$LSH_CONFIG_DIR/config.lsh` (if set)
2. `~/.config/lsh/config.lsh` (or platform config directory)
3. Fallback: `./config/config.lsh`

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

# Completion UI: "menu" (fuzzy dropdown) or "list"
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

# List registered hooks
$hooks.list()

# Clear all hooks for an event
$hooks.clear("chpwd")
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
