# Stargate Language Quick Reference

A concise reference guide for Stargate scripting language.

---

## Basic Syntax

### Comments
```stargate
# This is a comment
let x = 5;  # Inline comment
```

### Variables
```stargate
let name = "Alice";
let age = 30;
let is_active = true;
let result = none;
```

### String Interpolation
```stargate
let name = "World";
let greeting = "Hello, {name}!";
```

---

## Data Types

| Type | Example | Description |
|------|---------|-------------|
| String | `"hello"` | UTF-8 text |
| SmallInt | `42` | 32-bit integer |
| Number | `3.14` | 64-bit float |
| Bool | `true`, `false` | Boolean |
| None | `none` | Null value |
| List | `[1, 2, 3]` | Ordered collection |
| Dict | `{"key": "value"}` | Key-value pairs |
| Set | `set(1, 2, 3)` | Unique elements |

---

## Operators

### Arithmetic
```stargate
+  -  *  /  %    # add, subtract, multiply, divide, modulo
```

### Comparison
```stargate
==  !=  <  >  <=  >=
```

### Logical
```stargate
&&  ||  !        # and, or, not
```

### Pipeline
```stargate
|                # pipe data between operations
```

---

## Control Flow

### If Statement
```stargate
if condition {
    # code
} else {
    # code
}
```

### While Loop
```stargate
let i = 0;
while i < 10 {
    print i;
    let i = i + 1;
}
```

### For Loop
```stargate
# Over list
for item in [1, 2, 3] {
    print item;
}

# With index
for idx, value in list {
    print "{idx}: {value}";
}

# Over dictionary
for key, value in dict {
    print "{key} = {value}";
}
```

---

## Functions

### Definition
```stargate
fn add(a, b) {
    return a + b;
}
```

### Call
```stargate
let result = add(3, 4);
```

### Recursive
```stargate
fn factorial(n) {
    if n <= 1 { return 1; }
    return n * factorial(n - 1);
}
```

---

## Classes

### Basic Class
```stargate
class Person {
    let name = "Unknown";
    let age = 0;
}

let person = new Person;
print person.name;
```

### With Methods
```stargate
class Counter {
    let count = 0;
    
    fn increment() {
        let count = count + 1;
        return this;
    }
    
    fn get() {
        return count;
    }
}

let counter = new Counter;
counter.increment().increment();
print counter.get();  # 2
```

### Inheritance
```stargate
class Animal {
    let name = "Unknown";
}

class Dog extends Animal {
    let breed = "Mixed";
}
```

---

## Collections

### Lists
```stargate
let list = [1, 2, 3, 4, 5];
let first = list[0];
let last = list[-1];         # Python-style negative indexing
let size = list.size();
```

### Dictionaries
```stargate
let dict = {
    "name": "Alice",
    "age": 30
};
let name = dict["name"];
let keys = dict.keys();
let has_key = dict.contains("name");
```

### Sets
```stargate
let set1 = set(1, 2, 3);
let has_item = set1.contains(2);
let set2 = set1.insert(4);
let set3 = set1.remove(1);
```

---

## Closures

### Basic Syntax
```stargate
# Single parameter
let double = x: x * 2;

# Multiple parameters
let add = a, b: a + b;

# With pipes
let multiply = |a, b| a * b;
```

### Map
```stargate
let numbers = [1, 2, 3, 4, 5];
let doubled = numbers.map(x: x * 2);
# Result: [2, 4, 6, 8, 10]
```

### Filter
```stargate
let numbers = [1, 2, 3, 4, 5, 6];
let evens = numbers.filter(x: x % 2 == 0);
# Result: [2, 4, 6]
```

### Reduce
```stargate
let numbers = [1, 2, 3, 4, 5];
let sum = numbers.reduce(0, acc, x: acc + x);
# Result: 15
```

### Chaining
```stargate
let result = [1, 2, 3, 4, 5]
    .filter(x: x > 2)
    .map(x: x * 2)
    .reduce(0, sum, x: sum + x);
```

---

## Pipelines

### Basic Pipeline
```stargate
let result = expression | command;
```

### Object Pipeline
```stargate
let count = (list-directory) | slice-object count;
let entries = (list-directory) | slice-object entries;
```

### Chained Pipeline
```stargate
let result = data | command1 | command2 | command3;
```

---

## Command Integration

### Command Execution (Returns Object)
```stargate
let dir = (list-directory);
print dir.count;
print dir.entries[0].name;
```

### Execute Process (Returns String)
```stargate
let output = execute-process("/bin/ls", "-la");
let result = execute-process("/bin/sh", "-c", "echo hello");
```

### Common Commands
```stargate
(list-directory)    # List files with metadata
(uptime)            # System uptime
(date)              # Current date/time
(users)             # Logged in users
(whoami)            # Current user
(mktemp)            # Create temp file
(pathchk path)      # Validate path
```

---

## Testing

### Test Annotation
```stargate
use ut;

[test]
fn test_addition() {
    let result = 2 + 2;
    ut.assert_equals(result, 4, "2 + 2 should equal 4");
}
```

### Assertions
```stargate
ut.assert_equals(actual, expected, "message");
ut.assert_not_equals(actual, expected, "message");
ut.assert_true(condition, "message");
ut.assert_false(condition, "message");
```

### Test Execution
```stargate
use ut;

# ... test functions ...

print ut.stats;      # Print test statistics
exit(ut.healthy);    # Exit with test status (0 = pass, 1 = fail)
```

---

## Built-in Methods

### String Methods
```stargate
str.upper()
str.lower()
str.split(delimiter)
str.trim()
str.contains(substring)
str.replace(old, new)
```

### List Methods
```stargate
list.size()
list.push(item)
list.pop()
list.map(closure)
list.filter(closure)
list.reduce(initial, accumulator, item: expression)
```

### Dictionary Methods
```stargate
dict.size()
dict.keys()
dict.values()
dict.contains(key)
```

### Set Methods
```stargate
set.size()
set.contains(item)
set.insert(item)
set.remove(item)
```

---

## Common Patterns

### File Processing
```stargate
let files = (list-directory).entries;
let large_files = files
    .filter(f: f.size > 1024)
    .map(f: f.name);

for name in large_files {
    print "Large: {name}";
}
```

### Builder Pattern
```stargate
class Builder {
    let value = 0;
    
    fn set_value(v) {
        let value = v;
        return this;
    }
    
    fn add(x) {
        let value = value + x;
        return this;
    }
}

let result = new Builder.set_value(10).add(5);
```

### Data Transformation
```stargate
let data = [
    {"name": "Alice", "score": 85},
    {"name": "Bob", "score": 92},
    {"name": "Charlie", "score": 78}
];

let high_scorers = data
    .filter(d: d["score"] > 80)
    .map(d: d["name"]);
# Result: ["Alice", "Bob"]
```

### Error Handling
```stargate
fn safe_divide(a, b) {
    if b == 0 {
        return none;
    }
    return a / b;
}

let result = safe_divide(10, 2);
if result != none {
    print "Result: {result}";
} else {
    print "Division by zero";
}
```

---

## Shell Features

### Interactive Mode
```bash
$ stargate-shell
stargate> let x = 5
stargate> print x
5
stargate> cd /tmp
stargate> (list-directory).count
42
```

### Tab Completion
```bash
# Commands
stargate> l<TAB>
link  list-directory  ln  ls

# Properties
stargate> (list-directory).<TAB>
entries  count  recursive

# Variables
stargate> let my_var = 5;
stargate> my<TAB>
my_var
```

### Background Jobs
```bash
stargate> long-command &
Job [1] started in background
```

### History
```bash
# Persistent history in ~/.stargate_history
Ctrl+R    # Search history
Ctrl+P    # Previous command
Ctrl+N    # Next command
```

---

## Environment Variables

### Bytecode Compilation
```bash
export STARGATE_BYTECODE=1
```

---

## File Extensions

- `.sg` - Stargate script files

---

## Shebang

```stargate
#!/usr/bin/env stargate-shell

# Your script here
print "Hello from Stargate!";
```

---

## Best Practices

### Use Closures for Transformations
```stargate
# Good
let doubled = list.map(x: x * 2);

# Avoid manual loops when closures work
```

### Chain Operations
```stargate
# Good - declarative
let result = data
    .filter(x: x > 0)
    .map(x: x * 2)
    .reduce(0, sum, x: sum + x);

# Avoid - imperative
let sum = 0;
for item in data {
    if item > 0 {
        let sum = sum + (item * 2);
    }
}
```

### Use String Interpolation
```stargate
# Good
let name = "Alice";
print "Hello, {name}!";

# Avoid
print "Hello, " + name + "!";
```

### Return `this` for Method Chaining
```stargate
class Builder {
    fn set_x(val) {
        let x = val;
        return this;  # Enable chaining
    }
}
```

### Use None for Missing Values
```stargate
fn find_user(id) {
    if not_found {
        return none;
    }
    return user;
}

let user = find_user(123);
if user != none {
    # Process user
}
```

---

## Tips & Tricks

### Negative Indexing
```stargate
let list = [1, 2, 3, 4, 5];
print list[-1];   # Last element: 5
print list[-2];   # Second to last: 4
```

### Optional Semicolons
```stargate
# Both are valid
let x = 5;
let y = 10;

let a = 1
let b = 2
```

### Multiple Assignments on One Line
```stargate
let x = 1; let y = 2; let z = 3;
```

### Object Property Exploration
```stargate
# Use tab completion to discover properties
stargate> let dir = (list-directory);
stargate> dir.<TAB>
entries  count  recursive
```

---

## Common Errors

### Division by Zero
```stargate
# Error
let result = 10 / 0;

# Solution
if divisor != 0 {
    let result = 10 / divisor;
}
```

### Index Out of Bounds
```stargate
# Error
let list = [1, 2, 3];
let item = list[10];

# Solution
if index < list.size() {
    let item = list[index];
}
```

### Undefined Variable
```stargate
# Error
print unknown_variable;

# Solution
let unknown_variable = "default";
print unknown_variable;
```

---

**For complete details, see [SPECIFICATION.md](./SPECIFICATION.md)**
