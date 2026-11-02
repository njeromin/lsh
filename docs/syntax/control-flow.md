# Control Flow

`lsh` provides structured control flow statements including conditionals, loops, break/continue, and return statements.

---

## Table of Contents

- [If / Else If / Else](#if--else-if--else)
- [While Loops](#while-loops)
- [For..In Loops](#forin-loops)
- [Break & Continue](#break--continue)
- [Return](#return)

---

## If / Else If / Else
Conditions evaluate expressions. Truthy values include `true`, non-zero numbers, and non-empty strings/objects.

```lsh
let score = 85

if $score >= 90 {
    echo "Grade: A"
} else if $score >= 80 {
    echo "Grade: B"
} else {
    echo "Grade: C"
}
```

---

## While Loops
Executes its body repeatedly as long as the condition remains truthy:

```lsh
let count = 5

while $count > 0 {
    echo $count
    $count = $count - 1
}

echo "Liftoff!"
```

---

## For..In Loops
Iterates over elements of an array:

```lsh
let tools = ["rust", "cargo", "git"]

for tool in $tools {
    echo "Tool: " + $tool
}
```

You can also iterate over lines returned from a command:

```lsh
for file in $(ls).lines() {
    echo "Found file: " + $file
}
```

---

## Break & Continue
Control loop iterations:

- `break` exits the nearest loop immediately.
- `continue` skips the rest of the current iteration.

```lsh
for i in [1, 2, 3, 4, 5] {
    if $i == 2 {
        continue
    }
    if $i == 4 {
        break
    }
    echo $i
}
# Output:
# 1
# 3
```

---

## Return
Use `return` to return an early value from a function. Returning without an argument returns `null`.

```lsh
fn find_user(id) {
    if $id == 1 {
        return "Alice"
    }
    return null
}
```
