# Stargate Language Specification

**Version:** 1.0.0  
**Date:** December 6, 2025  
**Copyright:** © 2025 Dmitry Kalashnikov  
**License:** MIT

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Lexical Structure](#2-lexical-structure)
3. [Types and Values](#3-types-and-values)
4. [Variables and Declarations](#4-variables-and-declarations)
5. [Expressions](#5-expressions)
6. [Statements](#6-statements)
7. [Functions](#7-functions)
8. [Classes and Objects](#8-classes-and-objects)
9. [Collections](#9-collections)
10. [Closures and Functional Programming](#10-closures-and-functional-programming)
11. [Pipeline Operations](#11-pipeline-operations)
12. [Command Integration](#12-command-integration)
13. [Module System](#13-module-system)
14. [Testing Framework](#14-testing-framework)
15. [Error Handling](#15-error-handling)
16. [Standard Library](#16-standard-library)

---

## 1. Introduction

Stargate is a modern scripting language designed for shell automation, data processing, and system administration. It combines the expressiveness of Python with the power of Unix pipelines, featuring first-class support for objects, closures, and command execution.

### 1.1 Design Goals

- **Object-Oriented Shell**: Treat command outputs as structured objects
- **Functional Programming**: First-class closures with map/filter/reduce
- **Pipeline Integration**: Seamless Unix command integration
- **Type Flexibility**: Dynamic typing with optional type checking
- **Interactive REPL**: Modern shell with tab completion
- **Testing Built-in**: Native unit testing framework

### 1.2 File Extension

Stargate script files use the `.sg` extension.

### 1.3 Execution Modes

Stargate supports two execution modes:

1. **Script Mode**: Execute files with `stargate-shell script.sg`
2. **Interactive Mode**: Launch REPL with `stargate-shell`

---

## 2. Lexical Structure

### 2.1 Comments

```stargate
# Single-line comment

# Comments start with # and continue to end of line
let x = 5;  # Inline comment
```

### 2.2 Identifiers

Identifiers follow these rules:

- Start with letter or underscore: `[a-zA-Z_]`
- Continue with letters, digits, underscores, or hyphens: `[a-zA-Z0-9_-]*`
- Case-sensitive: `myVar` ≠ `myvar`

**Valid identifiers:**
```stargate
x
my_variable
list-directory
_private
MyClass
user123
```

**Reserved keywords:**
```stargate
let if else while for fn return class new this
print exec script use true false none bool
```

### 2.3 Operators

**Arithmetic operators:**
```
+  -  *  /  %
```

**Comparison operators:**
```
==  !=  <  >  <=  >=
```

**Logical operators:**
```
&&  ||  !
```

**Assignment operator:**
```
=
```

**Pipeline operator:**
```
|
```

**Property access:**
```
.
```

### 2.4 Delimiters

```
(  )    # Parentheses
{  }    # Braces
[  ]    # Brackets
,       # Comma
;       # Semicolon (optional in many contexts)
:       # Colon (for closures and dictionaries)
```

### 2.5 Whitespace

Whitespace (spaces, tabs, newlines) is generally insignificant except:
- To separate tokens
- Inside string literals
- For line continuation in interactive mode

---

## 3. Types and Values

### 3.1 Primitive Types

#### 3.1.1 String

UTF-8 encoded text with interpolation support:

```stargate
let name = "Alice";
let greeting = "Hello, {name}!";  # String interpolation
let multiline = "Line 1
Line 2";  # Multiline strings
```

#### 3.1.2 SmallInt

32-bit signed integers:

```stargate
let count = 42;
let negative = -10;
let zero = 0;
```

#### 3.1.3 Number

64-bit floating-point numbers:

```stargate
let pi = 3.14159;
let price = 19.99;
let scientific = 1.5e10;
```

#### 3.1.4 Bool

Boolean values:

```stargate
let is_active = true;
let is_closed = false;
```

#### 3.1.5 None

Represents absence of value:

```stargate
let result = none;
if result == none {
    print "No result";
}
```

### 3.2 Complex Types

#### 3.2.1 Object

JSON-like structured data from command outputs:

```stargate
let dir_info = (list-directory);
print dir_info.count;
print dir_info.entries[0].name;
```

#### 3.2.2 Instance

User-defined class instances:

```stargate
class Person {
    let name = "Unknown";
    let age = 0;
}

let person = new Person;
```

#### 3.2.3 Closure

First-class functions:

```stargate
let add = |a, b| a + b;
let result = add(3, 4);  # 7
```

### 3.3 Type Conversions

```stargate
# To boolean
bool(0)           # false
bool(1)           # true
bool("")          # false
bool("text")      # true
bool(none)        # false

# To string (implicit via interpolation)
let x = 42;
print "Value: {x}";  # "Value: 42"
```

---

## 4. Variables and Declarations

### 4.1 Variable Declaration

```stargate
let x = 10;
let name = "Alice";
let items = [1, 2, 3];
```

### 4.2 Variable Assignment

```stargate
let x = 5;
x = 10;  # Error: reassignment not allowed

# Use let for reassignment in same scope
let x = 5;
let x = 10;  # OK in Stargate
```

### 4.3 Scope Rules

Variables are function-scoped:

```stargate
let x = 1;

fn test() {
    let x = 2;  # New variable, shadows outer x
    print x;    # Prints 2
}

test();
print x;  # Prints 1
```

---

## 5. Expressions

### 5.1 Literals

```stargate
42              # Integer
3.14            # Float
"hello"         # String
true            # Boolean
false           # Boolean
none            # None
[1, 2, 3]       # List
{"a": 1}        # Dictionary
set(1, 2, 3)    # Set
```

### 5.2 Arithmetic Expressions

```stargate
let sum = 10 + 5;
let diff = 20 - 7;
let product = 4 * 8;
let quotient = 15 / 3;
let remainder = 17 % 5;

# Operator precedence: *, /, % > +, -
let result = 10 + 5 * 2;  # 20, not 30
```

### 5.3 Comparison Expressions

```stargate
10 == 10        # true
5 != 3          # true
7 < 10          # true
15 > 10         # true
5 <= 5          # true
8 >= 3          # true
```

### 5.4 Logical Expressions

```stargate
true && false   # false
true || false   # true
!true           # false

# Short-circuit evaluation
let result = false && expensive_call();  # expensive_call not executed
```

### 5.5 String Interpolation

```stargate
let name = "World";
let count = 42;
let message = "Hello, {name}! Count: {count}";
# Result: "Hello, World! Count: 42"
```

### 5.6 Property Access

```stargate
let obj = (list-directory);
let count = obj.count;
let first_name = obj.entries[0].name;
```

### 5.7 Method Calls

```stargate
let list = [1, 2, 3, 4, 5];
let size = list.size();
let doubled = list.map(x: x * 2);
```

### 5.8 Index Access

```stargate
let items = [10, 20, 30, 40];
let first = items[0];      # 10
let last = items[-1];      # 40 (Python-style negative indexing)
let second_last = items[-2];  # 30

let data = {"key": "value"};
let val = data["key"];     # "value"
```

---

## 6. Statements

### 6.1 Expression Statement

```stargate
print "Hello";
(list-directory);
my_function(arg);
```

### 6.2 If Statement

```stargate
if condition {
    # then block
}

if condition {
    # then block
} else {
    # else block
}

if x > 10 {
    print "Large";
} else if x > 5 {
    print "Medium";
} else {
    print "Small";
}
```

### 6.3 While Loop

```stargate
let i = 0;
while i < 10 {
    print i;
    let i = i + 1;
}
```

### 6.4 For Loop

```stargate
# Iterate over list
for item in [1, 2, 3] {
    print item;
}

# Iterate with index
for idx, value in [10, 20, 30] {
    print "Index {idx}: {value}";
}

# Iterate over dictionary
for key, value in {"a": 1, "b": 2} {
    print "{key} = {value}";
}
```

### 6.5 Return Statement

```stargate
fn add(a, b) {
    return a + b;
}

fn get_status() {
    if condition {
        return "active";
    }
    return "inactive";
}
```

### 6.6 Print Statement

```stargate
print "Hello, World!";
print variable;
print "Value: {variable}";
```

### 6.7 Exit Statement

```stargate
exit(0);       # Exit with success
exit(1);       # Exit with error
exit(code);    # Exit with variable code
```

### 6.8 Assert Statement

```stargate
assert x > 0, "x must be positive";
assert result == expected;
```

### 6.9 Use Statement

```stargate
use ut;  # Import unit testing module
use mymodule;
```

---

## 7. Functions

### 7.1 Function Definition

```stargate
fn greet(name) {
    return "Hello, {name}!";
}

fn add(a, b) {
    return a + b;
}

fn no_return() {
    print "Side effect only";
}
```

### 7.2 Function Calls

```stargate
let result = add(3, 4);
greet("Alice");
```

### 7.3 Recursive Functions

```stargate
fn factorial(n) {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

let fact5 = factorial(5);  # 120
```

### 7.4 Function Annotations

```stargate
[test]
fn test_addition() {
    let result = add(2, 3);
    assert result == 5;
}
```

---

## 8. Classes and Objects

### 8.1 Class Definition

```stargate
class Person {
    let name = "Unknown";
    let age = 0;
    let city = "Nowhere";
}
```

### 8.2 Instance Creation

```stargate
let person = new Person;
print person.name;  # "Unknown"
```

### 8.3 Property Access and Modification

```stargate
let person = new Person;
let person_name = person.name;

# Note: Properties are initialized from class defaults
# Direct modification after creation requires special handling
```

### 8.4 Methods

```stargate
class Calculator {
    let value = 0;
    
    fn add(x) {
        let value = value + x;
        return this;
    }
    
    fn multiply(x) {
        let value = value * x;
        return this;
    }
    
    fn get_result() {
        return value;
    }
}

let calc = new Calculator;
let result = calc.add(5).multiply(3).get_result();  # 15
```

### 8.5 The `this` Keyword

Inside methods, `this` refers to the current instance:

```stargate
class Counter {
    let count = 0;
    
    fn increment() {
        let count = count + 1;
        return this;
    }
    
    fn get_count() {
        return count;
    }
}
```

### 8.6 Inheritance

```stargate
class Animal {
    let name = "Unknown";
    let sound = "...";
    
    fn make_sound() {
        print "{name} says {sound}";
    }
}

class Dog extends Animal {
    let sound = "Woof!";
    let breed = "Mixed";
}

let dog = new Dog;
dog.make_sound();  # "Unknown says Woof!"
```

### 8.7 Builder Pattern

```stargate
class Pizza {
    let size = "medium";
    let toppings = set();
    
    fn large() {
        let size = "large";
        return this;
    }
    
    fn add_topping(topping) {
        let toppings = toppings.insert(topping);
        return this;
    }
}

let pizza = new Pizza.large().add_topping("pepperoni");
```

---

## 9. Collections

### 9.1 Lists

#### Creation
```stargate
let empty = [];
let numbers = [1, 2, 3, 4, 5];
let mixed = [1, "hello", true, 3.14];
let nested = [[1, 2], [3, 4]];
```

#### Access
```stargate
let first = numbers[0];      # 1
let last = numbers[-1];      # 5 (Python-style)
let second_last = numbers[-2];  # 4
```

#### Methods
```stargate
numbers.size()              # 5
numbers.push(6)             # Adds element
numbers.map(x: x * 2)       # Transform elements
numbers.filter(x: x > 2)    # Filter elements
numbers.reduce(0, acc, x: acc + x)  # Reduce to single value
```

### 9.2 Dictionaries

#### Creation
```stargate
let empty = {};
let person = {
    "name": "Alice",
    "age": 30,
    "city": "NYC"
};
```

#### Access
```stargate
let name = person["name"];
let age = person["age"];
```

#### Methods
```stargate
person.size()               # Number of keys
person.keys()               # List of keys
person.values()             # List of values
person.contains("name")     # Check if key exists
```

### 9.3 Sets

#### Creation
```stargate
let empty = set();
let numbers = set(1, 2, 3, 4, 5);
let from_list = set([1, 2, 2, 3]);  # {1, 2, 3}
```

#### Methods
```stargate
numbers.size()              # 5
numbers.contains(3)         # true
numbers.insert(6)           # Add element
numbers.remove(3)           # Remove element
```

---

## 10. Closures and Functional Programming

### 10.1 Closure Syntax

```stargate
# Single parameter
let double = x: x * 2;

# Multiple parameters
let add = a, b: a + b;

# Multiple parameters (explicit)
let multiply = |a, b| a * b;
```

### 10.2 Map

Transform each element:

```stargate
let numbers = [1, 2, 3, 4, 5];
let doubled = numbers.map(x: x * 2);
# Result: [2, 4, 6, 8, 10]

let names = ["alice", "bob", "charlie"];
let upper = names.map(n: n.upper());
```

### 10.3 Filter

Select elements matching a condition:

```stargate
let numbers = [1, 2, 3, 4, 5, 6];
let evens = numbers.filter(x: x % 2 == 0);
# Result: [2, 4, 6]

let scores = [45, 67, 82, 91, 58];
let passing = scores.filter(s: s >= 60);
# Result: [67, 82, 91, 58]
```

### 10.4 Reduce

Accumulate values:

```stargate
let numbers = [1, 2, 3, 4, 5];
let sum = numbers.reduce(0, acc, x: acc + x);
# Result: 15

let product = numbers.reduce(1, acc, x: acc * x);
# Result: 120

# Find maximum
let max_val = numbers.reduce(numbers[0], max, x: if x > max { x } else { max });
```

### 10.5 Chaining

```stargate
let scores = [45, 67, 82, 91, 58, 73, 88, 95];
let result = scores
    .filter(s: s >= 70)
    .map(s: s * s)
    .reduce(0, sum, s: sum + s);
# Sum of squared passing scores
```

### 10.6 Closures on Objects

```stargate
let files = (list-directory).entries;
let large_files = files.filter(f: f.size > 1024);
let names = large_files.map(f: f.name);
```

---

## 11. Pipeline Operations

### 11.1 Basic Pipelines

```stargate
# Pipe expression to command
let result = (list-directory) | slice-object entries;

# Chain multiple operations
let data = expression | command1 | command2;
```

### 11.2 Pipeline with Property Access

```stargate
let count = (list-directory) | slice-object count;
let names = (list-directory) | slice-object entries | collect-field name;
```

### 11.3 Object Piping

```stargate
let dir_info = (list-directory);
let entries = dir_info | slice-object entries;
let first_entry = entries[0];
```

### 11.4 Conditional Pipelines

```stargate
let result = if condition { 
    data | process-a 
} else { 
    data | process-b 
};
```

---

## 12. Command Integration

### 12.1 Command Execution

#### Parentheses Syntax (Returns Object)
```stargate
let result = (list-directory);
let uptime = (uptime);
let date = (date);
```

#### Execute-Process (Returns String)
```stargate
let output = execute-process("/bin/ls", "-la");
let result = execute-process("/bin/sh", "-c", "echo hello");
```

### 12.2 Command Objects

Commands return structured objects with properties:

```stargate
let dir = (list-directory);
print dir.count;           # Number of entries
print dir.entries[0].name; # First entry name
print dir.entries[0].size; # First entry size
print dir.entries[0].type; # File type
```

### 12.3 Shell Integration

```stargate
# Execute shell commands
let files = execute-process("/bin/sh", "-c", "find . -name '*.sg'");

# Process output
let lines = files.split("\n");
```

### 12.4 Background Jobs

In interactive mode:
```stargate
stargate> long-running-command &
Job [1] started in background
```

---

## 13. Module System

### 13.1 Import Modules

```stargate
use ut;        # Import unit testing module
use mymodule;  # Import custom module
```

### 13.2 Standard Modules

#### `ut` - Unit Testing
```stargate
use ut;

ut.assert_equals(actual, expected, "message");
ut.assert_true(condition, "message");
ut.assert_false(condition, "message");
ut.assert_not_equals(actual, expected, "message");

print ut.stats;     # Print test statistics
exit(ut.healthy);   # Exit with test status
```

---

## 14. Testing Framework

### 14.1 Test Annotation

```stargate
[test]
fn test_addition() {
    let result = add(2, 3);
    ut.assert_equals(result, 5, "2 + 3 should equal 5");
}
```

### 14.2 Test Assertions

```stargate
use ut;

[test]
fn test_example() {
    # Equality
    ut.assert_equals(actual, expected, "message");
    ut.assert_not_equals(actual, expected, "message");
    
    # Boolean
    ut.assert_true(condition, "message");
    ut.assert_false(condition, "message");
}
```

### 14.3 Test Execution

```stargate
use ut;

[test]
fn test_one() {
    ut.assert_true(true, "Always passes");
}

[test]
fn test_two() {
    ut.assert_equals(1 + 1, 2, "Math works");
}

# Print results and exit
print ut.stats;
exit(ut.healthy);
```

### 14.4 Test Statistics

```stargate
ut.stats        # Returns test statistics object
ut.healthy      # Returns 0 if all pass, 1 otherwise
```

---

## 15. Error Handling

### 15.1 Runtime Errors

```stargate
# Division by zero
let result = 10 / 0;  # Error: Division by zero

# Invalid index
let list = [1, 2, 3];
let item = list[10];  # Error: Index out of bounds

# Undefined variable
print unknown_var;    # Error: Undefined variable
```

### 15.2 Assert Statements

```stargate
assert condition, "Error message";

# Example
assert x > 0, "x must be positive";
assert result != none, "result cannot be none";
```

### 15.3 Exit Codes

```stargate
exit(0);    # Success
exit(1);    # General error
exit(code); # Custom exit code
```

---

## 16. Standard Library

### 16.1 Built-in Commands

#### File System
- `list-directory` - List directory contents with metadata
- `cd` - Change directory
- `pwd` - Print working directory
- `mktemp` - Create temporary file/directory

#### System Information
- `uptime` - System uptime information
- `date` - Current date/time
- `users` - Logged in users
- `whoami` - Current user

#### File Operations
- `pathchk` - Validate path names
- `basename` - Extract filename from path
- `dirname` - Extract directory from path

### 16.2 Built-in Functions

#### Type Conversion
```stargate
bool(value)     # Convert to boolean
```

#### Output
```stargate
print expression    # Print to stdout
```

#### Testing
```stargate
ut.assert_equals(actual, expected, message)
ut.assert_true(condition, message)
ut.assert_false(condition, message)
ut.assert_not_equals(actual, expected, message)
```

### 16.3 String Methods

```stargate
str.upper()         # Convert to uppercase
str.lower()         # Convert to lowercase
str.split(delim)    # Split string
str.trim()          # Remove whitespace
str.contains(sub)   # Check substring
str.replace(old, new)  # Replace substring
```

### 16.4 List Methods

```stargate
list.size()         # Get length
list.push(item)     # Add item
list.pop()          # Remove last item
list.map(closure)   # Transform elements
list.filter(closure)  # Filter elements
list.reduce(init, closure)  # Reduce to single value
```

### 16.5 Dictionary Methods

```stargate
dict.size()         # Get number of keys
dict.keys()         # Get list of keys
dict.values()       # Get list of values
dict.contains(key)  # Check if key exists
```

### 16.6 Set Methods

```stargate
set.size()          # Get number of elements
set.contains(item)  # Check membership
set.insert(item)    # Add element
set.remove(item)    # Remove element
```

---

## Appendix A: Grammar Summary

```ebnf
program         ::= statement*

statement       ::= var_decl | assignment | if_stmt | while_stmt | for_stmt
                  | function_def | class_def | return_stmt | print_stmt
                  | exit_stmt | assert_stmt | use_stmt | expr_stmt

var_decl        ::= "let" identifier "=" expression ";"?
assignment      ::= identifier "=" expression ";"?
if_stmt         ::= "if" expression block ("else" block)?
while_stmt      ::= "while" expression block
for_stmt        ::= "for" identifier ("," identifier)? "in" expression block
function_def    ::= annotation* "fn" identifier "(" params? ")" block
class_def       ::= "class" identifier ("extends" identifier)? "{" class_body "}"
return_stmt     ::= "return" expression ";"?
print_stmt      ::= "print" expression ";"?
exit_stmt       ::= "exit" "(" expression? ")" ";"?
assert_stmt     ::= "assert" expression ("," expression)? ";"?
use_stmt        ::= "use" identifier ";"?
expr_stmt       ::= expression ";"?

expression      ::= logical_or

logical_or      ::= logical_and ("||" logical_and)*
logical_and     ::= equality ("&&" equality)*
equality        ::= comparison (("==" | "!=") comparison)*
comparison      ::= additive (("<" | ">" | "<=" | ">=") additive)*
additive        ::= multiplicative (("+" | "-") multiplicative)*
multiplicative  ::= unary (("*" | "/" | "%") unary)*
unary           ::= ("!" | "-") unary | pipeline
pipeline        ::= postfix ("|" identifier)*
postfix         ::= primary ("." identifier | "[" expression "]" | "(" args? ")")*

primary         ::= literal | identifier | "(" expression ")"
                  | list_literal | dict_literal | set_literal
                  | closure | "new" identifier | command_output
                  | "this"

literal         ::= number | string | "true" | "false" | "none"
list_literal    ::= "[" (expression ("," expression)*)? "]"
dict_literal    ::= "{" (dict_pair ("," dict_pair)*)? "}"
set_literal     ::= "set" "(" (expression ("," expression)*)? ")"
closure         ::= params ":" expression
command_output  ::= "(" identifier ")"

annotation      ::= "[" identifier "]"
block           ::= "{" statement* "}"
params          ::= identifier ("," identifier)*
args            ::= expression ("," expression)*
dict_pair       ::= expression ":" expression
```

---

## Appendix B: Reserved Keywords

```
let      if       else     while    for      fn       return
class    new      this     print    exec     script   use
true     false    none     bool     assert   exit     extends
```

---

## Appendix C: Operator Precedence

From highest to lowest:

1. Property/Method access: `.`, `[]`, `()`
2. Unary: `!`, `-` (unary)
3. Multiplicative: `*`, `/`, `%`
4. Additive: `+`, `-`
5. Comparison: `<`, `>`, `<=`, `>=`
6. Equality: `==`, `!=`
7. Logical AND: `&&`
8. Logical OR: `||`
9. Pipeline: `|`
10. Assignment: `=`

---

## Appendix D: Example Programs

### Hello World
```stargate
print "Hello, World!";
```

### Factorial
```stargate
fn factorial(n) {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

print factorial(5);  # 120
```

### File Processing
```stargate
let files = (list-directory).entries;
let large_files = files
    .filter(f: f.size > 1024)
    .map(f: f.name);

for name in large_files {
    print "Large file: {name}";
}
```

### Class Example
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

---

## Appendix E: Bytecode Compilation

Stargate supports optional bytecode compilation for improved performance. Set the environment variable:

```bash
export STARGATE_BYTECODE=1
```

The bytecode compiler optimizes:
- Function calls
- Variable lookups
- Expression evaluation
- Control flow

---

## Appendix F: Shell Features

### Interactive Mode Features

1. **Tab Completion**
   - Commands
   - Variables
   - Properties
   - Directories

2. **History**
   - Persistent across sessions
   - Timestamped entries
   - Searchable with Ctrl+R

3. **Key Bindings**
   - Emacs-style editing
   - Ctrl+P/N for history navigation

4. **Background Jobs**
   - `command &` to run in background
   - Job control support

### Semicolon Flexibility

Semicolons are optional in most contexts:

```stargate
# Optional semicolons
let x = 5
print x
let y = 10

# Explicit semicolons
let a = 1; let b = 2;
```

### Negative Indexing

Python-style negative indexing:

```stargate
let list = [1, 2, 3, 4, 5];
print list[-1];   # 5
print list[-2];   # 4
```

---

**End of Specification**
