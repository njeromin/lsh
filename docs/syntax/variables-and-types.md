# Variables & Data Types

`lsh` provides high-level scripting types alongside shell command execution.

---

## Table of Contents

- [Variables & Scoping](#variables--scoping)
  - [Declaration (`let`)](#declaration-let)
  - [Assignment](#assignment)
  - [Scoping](#scoping)
- [Operators](#operators)
  - [Arithmetic](#arithmetic)
  - [Comparison](#comparison)
  - [Logical](#logical)
- [Data Types](#data-types)
  - [Strings](#strings)
  - [Numbers](#numbers)
  - [Booleans](#booleans)
  - [Arrays](#arrays)
  - [Objects & Maps](#objects--maps)
  - [Null](#null)
  - [Type Checking (`typeof`)](#type-checking-typeof)

---

## Variables & Scoping

### Declaration (`let`)

Variables are declared using the `let` keyword:

```lsh
let name = "lsh"
let count = 42
let is_ready = true
```

### Assignment

Variables can be assigned to using a leading `$`:

```lsh
$name = "new_value"
```

### Scoping

`lsh` uses lexical block scoping. Variables declared inside blocks (`{ ... }`), functions, or loops are local to that block and shadow outer variables:

```lsh
let x = 10

if true {
    let x = 20
    echo $x    # 20
}

echo $x        # 10
```

---

## Operators

### Arithmetic

- `+` — Addition or string concatenation
- `-` — Subtraction
- `*` — Multiplication
- `/` — Division (division by zero returns `null`)

### Comparison

- `==` — Equal to
- `!=` — Not equal to
- `<` — Less than
- `<=` — Less than or equal to
- `>` — Greater than
- `>=` — Greater than or equal to

### Logical

- `&&` — Logical AND
- `||` — Logical OR
- `!` — Logical NOT

---

## Data Types

### Strings

#### Double-Quoted Strings (Interpolated)

Double-quoted strings support variable interpolation (`$var`), command substitutions (`$(cmd)`), and escape characters:

```lsh
let user = "Alice"
echo "Hello, $user!"
echo "Current directory: $(pwd).out"
```

Supported escape sequences:

- `\n` — newline
- `\t` — tab
- `\r` — carriage return
- `\\` — backslash
- `\"` — double quote
- `\'` — single quote
- `\e` or `\x1b` — ANSI escape code

#### Single-Quoted Strings (Literal)

Single-quoted strings are literal and do not perform variable or command expansion:

```lsh
echo 'Literal $user and $(command)'
```

#### String Indexing & Methods

Strings support negative indexing (`[-1]` is the last character) and built-in helper methods:

```lsh
let s = "  Hello, World!  "

# Indexing & length
echo $s[0]                            # " "
echo $s[-1]                           # " "
echo $s.len()                         # 17 (or $s.length)

# Trimming
echo $s.trim()                        # "Hello, World!"
echo $s.trim_start()                  # "Hello, World!  "
echo $s.trim_end()                    # "  Hello, World!"

# Case conversion
echo $s.to_lowercase()                # "  hello, world!  "
echo $s.to_uppercase()                # "  HELLO, WORLD!  "

# Search & Replace
echo $s.replace("World", "lsh")       # "  Hello, lsh!  "
echo $s.contains("World")             # true
echo $s.starts_with("  Hello")        # true
echo $s.ends_with("!  ")              # true

# Splitting
let parts = $s.trim().split(", ")     # ["Hello", "World!"]
```

---

### Numbers

Numbers in `lsh` are 64-bit floating point numbers (`f64`).

```lsh
let a = 10
let b = 3.5
let sum = $a + $b        # 13.5
let diff = $a - 5        # 5
let prod = $a * 2        # 20
let quot = $a / 2        # 5
```

---

### Booleans

Booleans are `true` or `false`:

```lsh
let is_valid = true
let is_done = false

if $is_valid && !$is_done {
    echo "Processing"
}
```

---

### Arrays

Arrays are defined with square brackets `[...]` and can hold mixed types:

```lsh
let fruits = ["apple", "banana", "cherry"]

# Indexing (supports negative indices)
echo $fruits[0]                # "apple"
echo $fruits[-1]               # "cherry"
echo $fruits.len               # 3 (or $fruits.len())

# Array Methods
$fruits.push("mango")          # Appends "mango"
let last = $fruits.pop()       # Removes and returns "mango"
let csv = $fruits.join(", ")   # "apple, banana, cherry"
echo $fruits.contains("apple") # true
```

---

### Objects & Maps

Objects are key-value maps defined with curly braces `{ ... }`:

```lsh
let user = {
    name: "Alice",
    role: "admin",
    visits: 5
}

# Access via dot notation or bracket notation:
echo $user.name                # "Alice"
echo $user["role"]             # "admin"

# Modifying properties:
$user.visits = $user.visits + 1
$user["status"] = "online"
```

---

### Null

`null` represents an absent or uninitialised value:

```lsh
let empty = null
```

---

### Type Checking (`typeof`)

The `typeof` operator returns the type name as a string:

```lsh
echo typeof "hello"   # "string"
echo typeof 42        # "number"
echo typeof true      # "bool"
echo typeof [1, 2]    # "array"
echo typeof { a: 1 }  # "object"
echo typeof null      # "null"
```
