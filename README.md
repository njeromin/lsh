# lsh

`lsh` is a small non-POSIX compliant shell written in Rust. It supports command composition features like pipelines using `|`, `&&`/`||`, env-prefix assignments, and redirection using `->`.

![lsh demo](assets/demo.gif)

## Quick start

### With Cargo

```bash
cargo run
```

Run a script file:

```bash
cargo run ./examples/functions.lsh
```

### With Nix

Drop into the dev shell:

```bash
nix develop
```

Build with flake outputs:

```bash
nix build .#default
nix build .#x86_64-unknown-linux-gnu
nix build .#x86_64-unknown-linux-musl
nix build .#x86_64-pc-windows-gnu
```

## Language snapshot

```lsh
# variables
let name = "lsh"

# functions
fn greet(who) {
  return "hello, " + $who
}

# command substitution (returns object: code/out/err)
echo $(greet $name).out

# pipelines and chaining
echo "foo" | cat
missing-cmd || echo "fallback"

# env-prefix assignment (scoped to this command)
ENV_VAR=1 echo $ENV_VAR

# stdout redirection
echo "saved" -> /tmp/lsh-out.txt
```

## Builtins

Current builtins are:

- `alias` (`set`/`list`)
- `cd`
- `exec` (run a file in a fresh evaluator)
- `exit`
- `fmt-cwd`
- `source` (run a file in current evaluator)

## Config

At startup, `lsh` looks for `config.lsh` in:

- `$LSH_CONFIG_DIR/config.lsh` if `LSH_CONFIG_DIR` is set
- otherwise your platform config dir (`.../lsh/config.lsh`)
- fallback: `./config/config.lsh`

This repo includes a sample config at [`config/config.lsh`](./config/config.lsh) with prompt customisation and aliases.

## Development

Useful commands:

```bash
cargo check
cargo test -q
```

Optional parser/lexer debug output:

```bash
DEBUG_LEXER=1 cargo run -q
DEBUG_PARSER=1 cargo run -q
```

## Notes and current limits

- This is not a POSIX shell implementation.
- Redirection is handled differently to most shells using `->` instead of `>`.
