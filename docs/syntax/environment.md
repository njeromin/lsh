# Environment Variables

`lsh` provides integration with the operating system's process environment.

---

## Table of Contents

- [Accessing Environment Variables](#accessing-environment-variables)
- [Updating `$PATH`](#updating-path)
- [The `export` Builtin](#the-export-builtin)

---

## Accessing Environment Variables
All OS environment variables (such as `$PATH`, `$USER`, `$HOME`, and `$PWD`) are directly accessible in `lsh` using the `$` prefix:

```lsh
echo $USER
echo $HOME
echo $PATH
```

---

## Updating `$PATH`
Modifying `$PATH` automatically updates the operating system's process environment:

```lsh
# Prepend a directory to PATH:
$PATH = "/custom/bin:" + $PATH

# Or without the leading $:
PATH = "$HOME/.local/bin:$PATH"

# Or with let:
let PATH = "$HOME/.cargo/bin:$PATH"
```

### What happens when `$PATH` changes:
1. **Host Process Updated**: The shell updates the process environment via `std::env::set_var`.
2. **Binary Execution**: Any external commands can be run directly from the new path directories.
3. **Child Inheritance**: All spawned child commands and subshells inherit the updated `$PATH`.
4. **Dynamic Completion**: The line editor's binary tab-completion cache dynamically re-scans the new directories so newly added executables can be autocompleted immediately.

---

## The `export` Builtin

Use `export` to set or export variables to the environment:

```lsh
# Export key=value:
export EDITOR="nvim"

# Export key and value as separate arguments:
export PAGER "less"

# Export an existing local shell variable:
let MY_FLAG = "enabled"
export MY_FLAG

# List all exported environment variables:
export
```
