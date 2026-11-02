# `lsh` Syntax Documentation

`lsh` is a non-POSIX shell written in Rust. It combines traditional Unix pipeline and command execution with modern scripting language features like lexical scoping, first-class functions, object notation, and structured command substitution.

---

## Documentation Sections

Explore the documentation by topic:

| Topic | Description | Link |
| :--- | :--- | :--- |
| **Variables & Data Types** | `let` declarations, assignments, scoping, strings, numbers, booleans, arrays, objects, `null`, and `typeof` | [variables-and-types.md](syntax/variables-and-types.md) |
| **Commands & Pipelines** | Running external binaries, pipelines (`\|`), chaining (`&&`/`\|\|`), redirection (`->`), and command substitution (`$(...)`) | [commands-and-pipelines.md](syntax/commands-and-pipelines.md) |
| **Control Flow** | `if`/`else`, `while` loops, `for..in` loops, `break`, `continue`, and `return` | [control-flow.md](syntax/control-flow.md) |
| **Functions & Methods** | Named functions (`fn`), arrow closures, expression vs command calls, and object methods with `this` | [functions-and-methods.md](syntax/functions-and-methods.md) |
| **Environment Variables** | Accessing `$PATH`, automatic process synchronisation, and the `export` builtin | [environment.md](syntax/environment.md) |
| **Builtins & Configuration** | Shell builtins (`cd`, `alias`, `export`, etc.), `config.lsh`, `$prompt`, `$options`, `$palette`, and `$hooks` | [builtins-and-configuration.md](syntax/builtins-and-configuration.md) |

---

## Quick Syntax Cheat Sheet

```lsh
# Variables & Assignment
let name = "lsh"
name = "new_name"          # Reassignment without $
$name = "new_name"         # Reassignment with $

# Strings & Interpolation
let greeting = "Hello, $name!"
let raw = 'Literal $name'

# Data Types
let num = 42
let flag = true
let list = ["apple", "banana", "cherry"]
let map = { key: "value", count: 10 }

# Functions & Closures
fn add(a, b) {
    return $a + $b
}
let square = x => x * x

# Command Execution & Pipelines
cat file.txt | grep "pattern"
cargo check && cargo build
echo "hello" -> /tmp/out.txt      # Note: -> for redirection

# Command Substitution (Structured Result)
let branch = $(git branch --show-current).out.trim()
if $(which eza).is_ok() {
    alias set ls "eza --icons"
}

# Environment & PATH
$PATH = "$HOME/.local/bin:$PATH"
export EDITOR="nvim"
```
