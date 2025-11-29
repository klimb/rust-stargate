# Stargate 🌠
### A Modern UNIX Userland with Built-In Testing, Classes & Object Pipes

**Stargate** reimagines the UNIX command-line with structured data, intelligent tab completion, and a powerful scripting language featuring **built-in unit testing**, full OOP support, and type-safe object pipelines. No more text parsing—embrace object pipelines that are faster and infinitely more expressive.

## ✨ Why Stargate?

```bash
# Traditional UNIX: Parse, grep, awk, sed, repeat...
ls -la | grep "\.rs$" | awk '{print $9}' | wc -l

# Stargate: Clean, fast, type-safe object pipelines
list-directory | slice-object entries | collect-count
```

### 🎯 Key Features

#### **Built-In Unit Testing Framework**
First-class testing support built directly into the language:
```rust
#!/usr/bin/env stargate-shell
use ut;

class Calculator {
    fn add(a, b) { return a + b; }
    fn multiply(a, b) { return a * b; }
}

[test]
fn test_addition() {
    let calc = new Calculator;
    ut.assert_equals(calc.add(2, 3), 5, "Addition should work");
    ut.assert_equals(calc.add(-1, 1), 0, "Should handle negatives");
}

[test]
fn test_multiplication() {
    let calc = new Calculator;
    ut.assert_equals(calc.multiply(3, 4), 12, "Multiplication should work");
}

print ut.stats;
exit(ut.healthy);
```

**Running tests:**
```bash
$ ./target/debug/stargate-shell my_tests.sg

=== Running 2 test(s) ===

Running test: test_addition... ✓ PASSED
Running test: test_multiplication... ✓ PASSED

=== Test Results ===
Passed: 2, Failed: 0, Total: 2
```

**Testing assertions:**
- `ut.assert_equals(actual, expected, message)` - Compare values
- `ut.assert_not_equals(actual, expected, message)` - Assert inequality
- `ut.assert_true(condition, message)` - Assert boolean condition
- `ut.stats` - Get test statistics (Passed: X, Failed: Y, Total: Z)
- `ut.healthy` - Returns `true` if all tests passed, `false` otherwise

#### **Object-Oriented Programming with Classes**
Full class support with inheritance and methods:
```rust
class Animal {
    let name = "Unknown";
    let sound = "...";
    
    fn make_sound() {
        print "{name} says {sound}";
    }
}

class Dog extends Animal {
    let sound = "Woof";
    let breed = "Labrador";
    
    fn fetch() {
        print "{name} the {breed} is fetching!";
    }
}

let dog = new Dog;
dog.make_sound();  # "Unknown says Woof"
dog.fetch();       # "Unknown the Labrador is fetching!"
```

**Test your classes:**
```rust
[test]
fn test_inheritance() {
    let dog = new Dog;
    ut.assert_equals(dog.sound, "Woof", "Dog should override sound");
    ut.assert_equals(dog.name, "Unknown", "Dog should inherit name");
    ut.assert_equals(dog.breed, "Labrador", "Dog should have breed");
}
```

#### **Object Pipelines - No Text Parsing**
Commands output structured JSON, pipelines work on objects:
```bash
# Traditional shell - text parsing hell
bash$ ls -la | grep "\.rs$" | awk '{print $9}' | while read f; do echo $f; done

# Stargate - direct object manipulation
stargate> list-directory | slice-object entries | dice-object name size type

# Access properties directly
stargate> (list-directory).entries[0].name
"Cargo.toml"

# Complex pipelines with filtering
stargate> list-directory | find-text rust | slice-object entries | dice-object name permissions

# Test pipeline behavior
[test]
fn test_directory_listing() {
    let total = (list-directory).total_count;
    ut.assert_true(total > 0, "Directory should have items");
    
    let first_name = (list-directory).entries[0].name;
    ut.assert_not_equals(first_name, "", "First entry should have name");
}
```

#### **Intelligent Tab Completion**
Tab completion for **everything** - commands, properties, directories, variables:

```bash
# Command completion with aliases
stargate> l<TAB>
link  list-directory  ln  ls  ld

# Property exploration - discover available fields
stargate> (list-directory).<TAB>
entries      recursive    total_count

# Nested property completion
stargate> (list-directory).entries[0].<TAB>
gid  inode  modified  name  nlink  path  permissions  size  type  uid

# Directory completion for cd
stargate> cd sr<TAB>
stargate> cd src/

# Variable completion
stargate> let my_people = "go";
stargate> print my<TAB>
stargate> print my_people
```

#### **Stargate Shell - Interactive & Scriptable**
A modern shell with Python-style indexing and optional semicolons:

```bash
# Variables and expressions
stargate> let total = (list-directory).total_count;
stargate> print "Found {total} items";
Found 41 items

# Python-style negative indexing
stargate> (list-directory).entries[-1].name
".codecov.yml"

# No semicolons needed for simple commands
stargate> cd src
stargate> list-directory
stargate> cd ..

# Pipeline from variables
stargate> let cmd = list-directory;
stargate> cmd | slice-object entries | collect-count
```

#### **String Interpolation**
```bash
stargate> let user = (get-username).username;
stargate> let host = (get-hostname).hostname;
stargate> print "I am {user}@{host}";

# Works with any expression
stargate> print "Last file: {(list-directory).entries[-1].name}";
```

### 🚀 Language Features

**Functions with Testing:**
```rust
fn factorial(n) {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

[test]
fn test_factorial() {
    ut.assert_equals(factorial(5), 120, "5! should be 120");
    ut.assert_equals(factorial(0), 1, "0! should be 1");
}
```

**Loops and Ranges:**
```rust
fn sum_range(start, end) {
    let total = 0;
    for i in range(start, end + 1) {
        let total = total + i;
    }
    return total;
}

[test]
fn test_sum_range() {
    ut.assert_equals(sum_range(1, 10), 55, "Sum 1..10 should be 55");
    ut.assert_equals(sum_range(1, 5), 15, "Sum 1..5 should be 15");
}
```

**Conditionals with Strict Type Checking:**
```rust
# Type safety: integers require bool() conversion
let count = 5;
if bool(count) {
    print "Count is non-zero";
}

[test]
fn test_bool_conversion() {
    ut.assert_equals(bool(0), false, "0 should convert to false");
    ut.assert_equals(bool(1), true, "1 should convert to true");
    ut.assert_equals(bool(42), true, "Non-zero should be true");
}
```

**Logical Operators with Complex Expressions:**
```rust
fn check_access(is_admin, has_permission, is_authenticated) {
    # Complex boolean logic with De Morgan's laws
    if is_admin || (has_permission && is_authenticated) {
        return "granted";
    }
    return "denied";
}

[test]
fn test_access_control() {
    ut.assert_equals(check_access(true, false, false), "granted", "Admin bypasses checks");
    ut.assert_equals(check_access(false, true, true), "granted", "User with perms OK");
    ut.assert_equals(check_access(false, true, false), "denied", "Not authenticated");
}
```

**Pipeline Testing:**
```rust
[test]
fn test_complex_pipeline() {
    # Test multi-stage pipelines
    let toml_files = list-directory | find-text toml;
    let count = toml_files.total_count;
    ut.assert_true(count >= 0, "Should count TOML files");
    
    # Test property access on pipeline results
    let entries = (list-directory).entries;
    let first_type = entries[0].type;
    ut.assert_not_equals(first_type, "", "First entry should have type");
}
```

**Python-Style Indexing:**
```rust
let entries = (list-directory).entries;
let first = entries[0];
let last = entries[-1];  # Negative indexing from end
let slice = entries[0..3];  # Range slicing
```

### 🎨 Design Philosophy

- **Verb-Noun Naming**: `list-directory`, `get-hostname`, `set-permissions` - reads like English
- **Object Pipelines**: Structured data flows through commands, no text parsing needed
- **Intelligent Completion**: Tab completes commands, properties, directories, variables
- **Classes & OOP**: Full class support with inheritance for complex scripts
- **Consistent Parameters**: `-r` always means recursive, `-v` always means verbose, `-h` always shows help
- **Smart Aliases**: Auto-generated from multi-word commands (`list-directory` → `ld`)
- **Do One Thing Well**: Each command has a single, clear purpose

### 📦 Quick Start

```bash
git clone https://github.com/klimb/rust-stargate
cd rust-stargate
make

# Beautiful colored output with file type indicators (no flags needed!)
./target/debug/stargate list-directory

# Interactive shell with tab completion & object scripting
./target/debug/stargate-shell 

stargate> ls                           # Use alias (ls → list-directory)
stargate> cd src                       # Built-in cd (no semicolon needed!)
stargate> let files = (ls).entries;    # Capture command output
stargate> print files[0].name;         # Access object properties
stargate> files | slice-object | dice-object name size  # Pipeline objects
```

**Run the complete test suite:**
```bash
# All 31 demo scripts with 150+ tests
make test-scripting

# Example output:
Running /path/to/class_methods_demo.sg...
=== Running 9 test(s) ===
Running test: test_driver_first_name... ✓ PASSED
Running test: test_car_brand... ✓ PASSED
Running test: test_get_full_name... ✓ PASSED
...
Passed: 9, Failed: 0, Total: 9

All scripting tests passed!
```

**Write your own tested scripts:**
```bash
#!/usr/bin/env stargate-shell
use ut;

class MathUtils {
    fn is_prime(n) {
        if n <= 1 { return false; }
        if n == 2 { return true; }
        
        for i in range(2, n) {
            if n % i == 0 {
                return false;
            }
        }
        return true;
    }
}

[test]
fn test_prime_numbers() {
    let math = new MathUtils;
    ut.assert_equals(math.is_prime(2), true, "2 is prime");
    ut.assert_equals(math.is_prime(17), true, "17 is prime");
    ut.assert_equals(math.is_prime(18), false, "18 is not prime");
}

print ut.stats;
exit(ut.healthy);
```

Save as `my_test.sg`, make executable, and run:
```bash
chmod +x my_test.sg
./target/debug/stargate-shell my_test.sg
```

### 🔥 Tab Completion Examples

**Discover available properties:**
```bash
stargate> (list-directory).<TAB>
entries      recursive    total_count

stargate> (list-directory).entries[0].<TAB>
gid  inode  modified  name  nlink  path  permissions  size  type  uid
```

**Complete directories for cd:**
```bash
stargate> cd <TAB>
docs/  examples/  src/  target/  tests/

stargate> cd sr<TAB>
stargate> cd src/
```

**Command and alias completion:**
```bash
stargate> l<TAB>
link  list-directory  ln  ls  ld

stargate> c<TAB>
cat  cd  change-directory  cksum  chmod  chown  chroot  ...
```

### Platform Support

- FreeBSD
- OpenBSD  
- GNU/Linux
- macOS

---

# Stargate Manifesto

- [UNIX userland was always as mess, you're just used to it](https://www.linkedin.com/pulse/unix-userland-always-mess-youre-just-used-dmitry-kalashnikov-2k6sc)
- ever wondered why its rm -rf, yet its chown -Rf user:group? ls ("list" what? I think you mean directory files .. etc). Why does "rm" also handle recursive removal of sub-directories, when its supposed to just "remove directory entries"? Why do we need "rmdir -p a/b/c" to duplicate this ("recursively" removes empty directories only)? why is it -p (instead of r)? Better name and parameter: remove-directory -r 
- standardizing UNIX "userland" (commands you type) naming with verb-noun and their parameters (-h always means help, -v verbose and so on). Its obvious that some parameters are common, some unique per command. Needs a thin parameter parsing
 layer. And structured (command) output for selection instead of searching through text streams (super slow, big-O). This is also a common parameter.
- some commands are focused on doing one thing and doing it well, and can be expressed as a verb-noun: ls is list-directory. Other commands (already) handle multiple verbs: hostname (hostname: "set or print name of current host system"). They can be split into set-hostname and get-hostname commands (disk space is not a concern in 2025). Or they need to be noun verb instead of verb noun: freebsd-update fetch (already does that .. that what we want). Another good example: "pkg update". There is going to be a noun and a verb (or vise-versa).
- aliases are two different things: (1) short names for longer commands and (2) their default params: some-long-command is slc. Convention over configuration.
- Rust is infinitely superior to C for implementing a new UNIX userland. C is an ancient procedural language for working with bare metal and kernel -- the userland code requires higher levels of abstraction, memory safety, OO, functional idioms, ability to leverage design patterns, ddd, built-in support for unit testing, internationalization, etc. Also rust has better error handling, support for modules, its much more expressive, enums and patterns, traits and generics, closures, iterators, collections, infinitely better strings and text handling, concurrency, async idioms, macros, etc.

## Non-Goals
- supporting UNIX POSIX compatibility .. using legacy ways of interacting with UNIX through a command-line interface and operating on unstructured streams of text.
- supporting Windows compatibility (just use Windows Powershell instead). Its kind of ridiculous that every command in (rust) coreutils was handling how Windows works (and Android, and SE Linux, and a, and b, etc). No one that runs Windows cares about coreutils. 
- supporting SELinux and Android.

## Goals
- reduce it to OpenBSD-style crystalized essentials. The BSD userland (compared to GNU/CoreUtils, including rust rewrite) is much much smaller (by 10x if its OpenBSD). And much cleaner, and significantly easier to read, find bugs and security problems. Smaller is better (do one thing).
- Build from there: split all commands into verbs. So they do one thing and do it well. Right now they're doing a lot of things. Renaming all commands to verb-noun to reduce mental friction. So it sounds just like English. The fewer command-line arguments, the better. Ideally get-[nount] would have zero parameters. Ideally [mutator]-[noun] would just have one argument. Smaller and simpler is better. Reads like math proof instead of convoluted procedural branching (this is mostly because of C, no better way to express yourself).
- Shortnames are a convention (and conflict resolution rules): some-long-command is slc. Stargate shell will know how to do this. You don't need to read a 500 page manual to use an iPhone or any modern GUI. We've come a long way. The command-line interface hasn't advanced -- it is stuck in the 80's (POSIX). Because it works. To be specific, the part that works is being able to speak commands to a machine (in a very primitive way). It will involve a noun and a verb (or vise-versa).
- Standardize on input parameters (don't care about legacy POSIX): -r is always recursive, -v is always verbose ... etc. Stuff works as expected. A command that takes lots of parameters is doing too much, and branches too much. Its that simple.
- Introduce a super thin object layer (for optional object output), so piping is faster by an order of magnitude (in some cases). Instead of searching in unstructured streams of text output (such as stdout), it will be a selection, slicing and dicing. Monad / MS Powershell has awesome design ideas. They came from UNIX ideas. Some UNIX ideas: (1) Do one thing and do it well. (2) Everything is a file (including pipes, stdout (just a special file), stderr (its also just a special file), directories are files, pipes and sockets are also files, etc). (3) Commands can be chained together with pipes. Output of one becomes an input into another (so far its been done as unstructured text; stderr is a design flaw).
- Some stats: Rust is way better for this than C (by like 1 million percent, give or take).
- Target platforms: FreeBSD, OpenBSD, GNU/Linux & Mac OS X.

Its much easier to do AI magic when your stuff speaks ultra simple English. 
