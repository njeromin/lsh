# Commands, Pipelines & Redirection

`lsh` provides command execution, piping, logical operators, file redirection, and structured command substitution.

---

## Table of Contents

- [Command Execution](#command-execution)
- [Pipelines (`|`)](#pipelines-)
- [Logical Chaining (`&&` / `||`)](#logical-chaining---)
- [Output Redirection (`->`)](#output-redirection--)
- [Prefix Environment Assignments](#prefix-environment-assignments)
- [Command Substitution (`$(...)`)](#command-substitution-)

---

## Command Execution
External programs are executed directly:

```lsh
ls -la
git status
cargo test -q
```

Arguments expand wildcards (globs) and tildes (`~`):

```lsh
cat ~/notes/*.txt
ls src/*.rs
```

---

## Pipes (`|`)
Connect the standard output of one command to the standard input of the next:

```lsh
cat /etc/passwd | grep "/bin/bash" | cut -d ":" -f 1
```

Pipelines stream data between stages and return the exit code of the final stage.

---

## Logical Chaining (`&&` / `||`)
Chain commands conditionally based on exit codes:

- `cmd1 && cmd2`: runs `cmd2` only if `cmd1` succeeds (exit status 0).
- `cmd1 || cmd2`: runs `cmd2` only if `cmd1` fails (non-zero exit status).

```lsh
cargo check && cargo test
which nonexistent_tool || echo "Tool not installed"
```

---

## Output Redirection (`->`)

> [!IMPORTANT]
> `lsh` uses `->` for standard output redirection instead of the POSIX `>` operator.

```lsh
echo "saved text" -> /tmp/output.txt
ls -la -> dir_contents.txt
```

---

## Prefix Environment Assignments
Variables prefixed before a command are scoped strictly to that command execution and do not leak into the shell state:

```lsh
DEBUG=1 cargo run
RUST_BACKTRACE=full my_binary
```

---

## Command Substitution (`$(...)`)
Command substitution captures the output of a command and returns a structured **Command Result Object** instead of a bare string:

### Result Object Properties & Methods:
- `.code` — Numeric exit code (`0` on success)
- `.out` — Standard output as a string
- `.err` — Standard error as a string
- `.is_ok() — Boolean (`true` if exit code is 0)
- `.is_err() — Boolean (`true` if exit code is non-zero)
- `.lines()` — Array of stdout lines

### Examples:

```lsh
# Check command success
if $(which eza).is_ok() {
    alias set ls "eza --icons"
}

# Capture trimmed stdout
let branch = $(git branch --show-current).out.trim()
echo "Current branch: $branch"

# Iterate over lines of output
for line in $(git status --porcelain).lines() {
    echo "Changed: " + $line
}

# Check error output
let res = $(git status)
if $res.is_err() {
    echo "Git error: " + $res.err
}
```
