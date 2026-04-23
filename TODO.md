# TODO

## Security

### High Priority

- [ ] Prevent child process I/O deadlocks in [src/eval/cmds.rs](./src/eval/cmds.rs)
  Current process handling drains `stdin`, `stdout`, and `stderr` sequentially. A child that fills one pipe while the parent is blocked on another can hang the shell.
  Relevant code: [src/eval/cmds.rs:35](./src/eval/cmds.rs:35), [src/eval/cmds.rs:67](./src/eval/cmds.rs:67), [src/eval/cmds.rs:73](./src/eval/cmds.rs:73), [src/eval/cmds.rs:79](./src/eval/cmds.rs:79)

- [ ] Replace panic-based runtime failures with diagnostics
  User-controlled scripts can still abort the whole process through runtime panics instead of receiving controlled errors.
  Relevant code: [src/eval/mod.rs:731](./src/eval/mod.rs:731), [src/eval/mod.rs:744](./src/eval/mod.rs:744), [src/eval/mod.rs:750](./src/eval/mod.rs:750), [src/ui/mod.rs:8](./src/ui/mod.rs:8), [src/diagnostics/mod.rs:107](./src/diagnostics/mod.rs:107)

- [ ] Add bounds checks for string repetition
  `"abc" * n` can allocate arbitrarily large strings and become an easy local DoS through memory exhaustion or capacity overflow.
  Relevant code: [src/eval/mod.rs:941](./src/eval/mod.rs:941), [src/eval/mod.rs:944](./src/eval/mod.rs:944)

### Medium Priority

- [ ] Decide and document the config-file trust model
  `LSH_CONFIG_DIR` fully controls which `config.lsh` is executed on startup. That is acceptable for a personal shell, but risky in automation, tests, or elevated contexts.
  Relevant code: [src/main.rs:144](./src/main.rs:144), [src/main.rs:158](./src/main.rs:158), [src/eval/exec.rs:5](./src/eval/exec.rs:5)

- [ ] Add a startup mode that skips config loading
  A `--no-config` or `--norc` mode would provide a safer entrypoint for scripts, testing, and debugging.
  Relevant code: [src/main.rs:136](./src/main.rs:136)

- [ ] Add a recursion/depth guard for user functions
  Deep or accidental recursion can overflow the Rust stack and abort the process.
  Relevant code: [src/eval/mod.rs:794](./src/eval/mod.rs:794), [src/eval/mod.rs:808](./src/eval/mod.rs:808)

## Performance

### High Priority

- [ ] Stop buffering entire pipelines in memory
  Pipeline stages currently materialize full byte buffers between commands instead of streaming. Large outputs will scale poorly and amplify latency and memory usage.
  Relevant code: [src/eval/mod.rs:555](./src/eval/mod.rs:555), [src/eval/mod.rs:635](./src/eval/mod.rs:635)

- [ ] Stop buffering full command-substitution output in memory
  `$(...)` currently captures full `stdout` and `stderr` into `Vec<u8>` and only then converts to strings.
  Relevant code: [src/eval/mod.rs:1006](./src/eval/mod.rs:1006), [src/eval/mod.rs:1028](./src/eval/mod.rs:1028), [src/eval/mod.rs:1096](./src/eval/mod.rs:1096)

### Medium Priority

- [ ] Reduce deep cloning in variable/object lookup
  Variable reads currently clone whole `Value`s, including objects and functions. This will become expensive once structured data usage grows.
  Relevant code: [src/eval/value.rs:34](./src/eval/value.rs:34), [src/eval/mod.rs:696](./src/eval/mod.rs:696), [src/eval/mod.rs:754](./src/eval/mod.rs:754), [src/eval/value.rs:106](./src/eval/value.rs:106)

- [ ] Avoid full object reconstruction during nested member assignment
  Object member assignment rebuilds owned maps recursively instead of mutating in place, which is costly for nested structures.
  Relevant code: [src/eval/mod.rs:736](./src/eval/mod.rs:736), [src/eval/mod.rs:747](./src/eval/mod.rs:747)

- [ ] Avoid parsing command-like statements twice
  Statements beginning with `String` or `Variable` tokens are often parsed once as expressions and then reparsed as commands after backtracking.
  Relevant code: [src/parser/mod.rs:78](./src/parser/mod.rs:78), [src/parser/mod.rs:113](./src/parser/mod.rs:113)

- [ ] Make diagnostics cheaper for repeated failures
  Diagnostic rendering rescans source text multiple times and allocates underline buffers for each report.
  Relevant code: [src/diagnostics/mod.rs:118](./src/diagnostics/mod.rs:118), [src/diagnostics/mod.rs:125](./src/diagnostics/mod.rs:125)

## Memory

### High Priority

- [ ] Add explicit output-size limits or failure modes for captured process output
  Captured `stdout` and `stderr` are stored in unbounded `Vec<u8>` buffers. Large or malicious subprocess output can exhaust memory.
  Relevant code: [src/eval/mod.rs:522](./src/eval/mod.rs:522), [src/eval/mod.rs:525](./src/eval/mod.rs:525), [src/eval/mod.rs:526](./src/eval/mod.rs:526), [src/eval/mod.rs:1030](./src/eval/mod.rs:1030), [src/eval/mod.rs:1066](./src/eval/mod.rs:1066)

- [ ] Revisit full-file loading for script execution
  Scripts are currently read fully into memory before lexing/parsing. Fine for small scripts, but it sets a hard scaling ceiling and makes resource abuse easier.
  Relevant code: [src/eval/exec.rs:8](./src/eval/exec.rs:8)

### Medium Priority

- [ ] Reduce temporary string churn in command evaluation
  Command identifiers, redirect targets, env assignment values, and command-sub results are repeatedly converted with `to_string()` and copied around.
  Relevant code: [src/eval/mod.rs:436](./src/eval/mod.rs:436), [src/eval/mod.rs:576](./src/eval/mod.rs:576), [src/eval/mod.rs:596](./src/eval/mod.rs:596), [src/eval/mod.rs:779](./src/eval/mod.rs:779), [src/eval/mod.rs:1016](./src/eval/mod.rs:1016)

- [ ] Review object/string display for unnecessary allocations
  `Display` for `Value` builds output through repeated string conversions and cloning, which will get more expensive once values become larger.
  Relevant code: [src/eval/value.rs:44](./src/eval/value.rs:44), [src/eval/value.rs:55](./src/eval/value.rs:55), [src/eval/value.rs:96](./src/eval/value.rs:96)

## Reliability

### High Priority

- [ ] Convert internal `expect`/`unwrap` usage on runtime-facing paths into recoverable errors
  Even when not directly user-triggered today, these are brittle failure points in a shell process.
  Relevant code: [src/main.rs:150](./src/main.rs:150), [src/main.rs:158](./src/main.rs:158), [src/main.rs:168](./src/main.rs:168), [src/diagnostics/mod.rs:116](./src/diagnostics/mod.rs:116), [src/ui/mod.rs:14](./src/ui/mod.rs:14)

### Medium Priority

- [ ] Improve alias handling to avoid naive space splitting
  Alias expansion currently uses `split(" ")`, which will behave badly with multiple spaces, quoting, or future shell syntax features.
  Relevant code: [src/eval/mod.rs:244](./src/eval/mod.rs:244), [src/eval/mod.rs:248](./src/eval/mod.rs:248)

- [ ] Clarify path canonicalization behavior for non-existent paths
  `expand_path` canonicalizes when possible and silently falls back otherwise. That is pragmatic, but error semantics are fuzzy and worth documenting or tightening.
  Relevant code: [src/paths.rs:5](./src/paths.rs:5), [src/paths.rs:9](./src/paths.rs:9)

## Validation

- [ ] Add regression tests for runtime panic cases and convert them to diagnostics
  Start with non-object member assignment and other currently panicking runtime paths.

- [ ] Add stress tests for large command substitution and pipeline output
  This will make memory regressions visible before they reach users.

- [ ] Add subprocess tests that exercise concurrent `stdout` and `stderr`
  This is needed to catch deadlocks once process I/O is reworked.
