# Functions & Methods

Functions in `lsh` are first-class values that can be defined by name, written as closures/arrow functions, or bound as methods on objects.

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
Invoke a function like a shell command (arguments separated by spaces):

```lsh
echo $(greet "lsh").out
```

---

## Anonymous & Arrow Functions
Arrow functions can be assigned to variables, stored in arrays/objects, or passed into hooks:

### Single Expression Syntax:
```lsh
let square = x => x * x
echo square(5)   # 25
```

### Block Body Syntax:
```lsh
let multiply = (a, b) => {
    return $a * $b
}
echo multiply(4, 5)  # 20

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
    count: 0,
    increment: () => {
        $this.count = $this.count + 1
        return $this.count
    },
    get_count: () => {
        return $this.count
    }
}

counter.increment()
counter.increment()
echo counter.get_count()   # 2
```
