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

Variables, object fields, and array elements can be reassigned using a leading `$` with `=` or compound assignment operators (`+=`, `-=`, `*=`, `/=`):

```lsh
$name = "new_value"
$count += 1
$count *= 2
$name += "!"
$obj.score -= 5
$arr[0] /= 2
```

### Scoping

`lsh` uses dynamic scoping with block- and function-local scopes. `let` always declares a variable in the innermost active scope, while reassignment updates the nearest enclosing active scope on the call stack (or defines it in the current scope if not found). Functions do not capture lexical environments:

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

- `+` — Addition (`Number + Number`), string concatenation (`String + any`), or array concatenation/append (`Array + Array`, `Array + any`)
- `-` — Subtraction and unary negation (`-$x`, `-(1 + 2)`)
- `*` — Multiplication (`Number * Number`) or string repetition (`String * Number`)
- `/` — Division (division by zero returns `null`)
- `+=`, `-=`, `*=`, `/=` — Compound assignment operators

### Comparison

- `==` — Equal to
- `!=` — Not equal to
- `<` — Less than
- `<=` — Less than or equal to
- `>` — Greater than
- `>=` — Greater than or equal to

### Logical

- `&&` — Logical AND (in expressions, evaluates both operands and returns a boolean)
- `||` — Logical OR (in expressions, evaluates both operands and returns a boolean)
- `!` — Logical NOT (`!$flag`, returns a boolean)

---

## Data Types

### Strings

#### Double-Quoted Strings (Interpolated)

Double-quoted strings support variable interpolation (`$var` for alphabetic/underscore names and special variables like `$?`, `$$`, `$!`), expression interpolation (`${expr}`), command substitutions (`$(cmd)`), and escape characters:

```lsh
let user = "Alice"
echo "Hello, $user!"
echo "Current directory: $(pwd)"
echo "Sum: ${1 + 2}"
```

Note that `$(cmd)` inside a double-quoted string automatically interpolates `cmd`'s stdout with trailing newlines trimmed.

Supported escape sequences:

- `\n` — newline
- `\t` — tab
- `\r` — carriage return
- `\0` — NUL byte
- `\\` — backslash
- `\"` — double quote
- `\'` — single quote
- `\$` — literal `$`

#### Single-Quoted Strings

Single-quoted strings do not perform variable (`$var`) or command (`$(...)`) expansion, while still interpreting basic backslash escapes (`\n`, `\r`, `\t`, `\0`, `\\`, `\'`):

```lsh
echo 'Literal $user and $(command)'
```

#### String Indexing & Methods

Strings support negative indexing (`[-1]` is the last character) and built-in helper methods:

```lsh
let s = "  Hello, World!  "

# Indexing & length
let first = $s[0]                     # " "
let last = $s[-1]                     # " "
let n = $s.len()                      # 17 (or $s.length / $s.length())

# Trimming
let trimmed = $s.trim()               # "Hello, World!"
let left = $s.trim_start()            # "Hello, World!  "
let right = $s.trim_end()             # "  Hello, World!"

# Case conversion
let lower = $s.to_lowercase()         # "  hello, world!  "
let upper = $s.to_uppercase()         # "  HELLO, WORLD!  "

# Search & Replace
let replaced = $s.replace("World", "lsh") # "  Hello, lsh!  "
let has_world = $s.contains("World")      # true
let starts = $s.starts_with("  Hello")    # true
let ends = $s.ends_with("!  ")            # true

# Splitting & JSON
let parts = $s.trim().split(", ")     # ["Hello", "World!"]
let parsed = '{"a": 1}'.from_json()   # { a = 1 }
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
let neg = -$a            # -10
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
let first = $fruits[0]             # "apple"
let last = $fruits[-1]             # "cherry"
let count = $fruits.len            # 3 (or $fruits.len())

# Array Methods
$fruits.push("mango")              # Appends "mango"
let popped = $fruits.pop()         # Removes and returns "mango"
let csv = $fruits.join(", ")       # "apple, banana, cherry"
let has_apple = $fruits.contains("apple") # true
let arr_json = $fruits.to_json()   # '["apple","banana","cherry"]'
```

---

### Objects & Maps

Objects are key-value maps defined with curly braces `{ ... }` using `=` between keys and values:

```lsh
let user = {
    name = "Alice",
    role = "admin",
    visits = 5
}

# Access via dot notation or bracket notation:
echo $user.name                # "Alice"
echo $user["role"]             # "admin"

# Modifying & serialising properties:
$user.visits += 1
$user["status"] = "online"
let serialized = $user.to_json(true)
```

---

### Null

`null` is returned for absent or uninitialised values (such as reading an undefined variable `$missing`, division by zero `1 / 0`, or returning without a value; note that `null` itself is not a keyword literal):

```lsh
let empty = $missing_var   # null
let div_zero = 1 / 0       # null
```

---

### Type Checking (`typeof`)

The `typeof` operator returns the type name as a string:

```lsh
let t1 = typeof "hello"    # "string"
let t2 = typeof 42         # "number"
let t3 = typeof true       # "bool"
let t4 = typeof [1, 2]     # "array"
let t5 = typeof { a = 1 }  # "object"
let t6 = typeof $missing   # "null"
echo $(typeof "hello").out # "string"
```
