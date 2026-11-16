# Functions & Methods

Functions in `lsh` are first-class values that can be defined by name, written as anonymous arrow functions, or bound as methods on objects. Functions use dynamic scoping and do not capture lexical closures.

---

## Table of Contents

- [Named Functions (`fn`)](#named-functions-fn)
- [Calling Styles](#calling-styles)
- [Anonymous & Arrow Functions](#anonymous--arrow-functions)
- [Object Methods & `this`](#object-methods--this)

---

## Named Functions (`fn`)
Named functions are defined using the `fn` keyword:

```lsh
fn greet(who) {
    return "Hello, " + $who
}
```

Functions can accept multiple parameters and return any value:

```lsh
fn add(a, b) {
    return $a + $b
}
```

---

## Calling Styles

`lsh` supports two ways to call functions:

### 1. Expression-style
Call a function with parentheses:

```lsh
let result = add(10, 20)
echo $result   # 30
```

### 2. Command-style
Invoke a function like a shell command (arguments separated by spaces); calling a function via `$(...)` captures its return value in `.out`:

```lsh
echo $(greet "lsh").out
```

---

## Anonymous & Arrow Functions
Arrow functions can be assigned to variables, stored in arrays/objects, or passed into hooks. Functions do not implicitly return the final expression; use `return` to return a value (otherwise `null` is returned):

### Single-Statement Body:
```lsh
let square = x => return $x * $x
let sq = square(5)
echo $sq                     # 25
echo $(square 5).out         # 25
```

### Block Body Syntax:
```lsh
let multiply = (a, b) => {
    return $a * $b
}
let product = multiply(4, 5)
echo $product                # 20

let log_message = () => {
    echo "Task complete"
}
log_message()
```

---

## Object Methods & `this`
When a function is stored as an object property, it acts as a method. The receiver object is accessible via `$this`:

```lsh
let counter = {
    count = 0,
    increment = () => {
        $this.count += 1
        return $this.count
    },
    get_count = () => {
        return $this.count
    }
}

$counter.increment()
$counter.increment()
let total = $counter.get_count()
echo $total                  # 2
```
