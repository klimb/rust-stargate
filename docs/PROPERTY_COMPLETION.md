# Property Completion in Stargate Shell

The stargate-shell REPL supports intelligent property completion for commands that output JSON objects.

## How to Use

**Important**: This is an interactive REPL feature. You must:
1. Start the shell: `./target/debug/stargate-shell`
2. Type a command followed by `.`
3. **Press TAB** (the Tab key on your keyboard)
4. See the property completions appear

Example session:
```
$ ./target/debug/stargate-shell
stargate> get-hostname.     ← Press TAB here
flags      hostname          ← Completions appear
stargate> (get-hostname).hostname
theone
```

## Features

### 1. Automatic Parenthesis Wrapping
When you type a command followed by `.`, the shell automatically wraps it in parentheses if needed.

Example:
```
stargate> get-hostname.<TAB>
```
Becomes:
```
stargate> (get-hostname).hostname
```

### 2. Property Suggestions
After typing `.`, press TAB to see available properties from the command's JSON output.

For `get-hostname --obj`:
- `flags` - Command flags used
- `hostname` - The hostname value

For `list-directory --obj`:
- `entries` - Array of directory entries
- `recursive` - Whether listing was recursive
- `total_count` - Total number of entries

### 3. Works with Existing Parentheses
If you already have parentheses, completion works normally:

```
stargate> (get-hostname).<TAB>
        flags    hostname
```

## Usage Examples

### Basic Property Access
```
stargate> script (get-hostname).hostname
theone
```

### Accessing Nested Properties
```
stargate> script (list-directory).total_count
41
```

### In Variable Assignments
```
stargate> script let host = (get-hostname).hostname
stargate> script print host
theone
```

## How It Works

1. When you type `.` after a command name, the completion system:
   - Detects the command before the dot
   - Executes it with `--obj` flag to get JSON output
   - Parses the JSON and extracts top-level property names
   - Provides those as completion candidates

2. If the command isn't already in parentheses:
   - Automatically wraps it: `command.` → `(command).`
   - Places cursor after the dot for property completion

## Supported Commands

Any stargate command that supports `--obj` flag will work with property completion, including:
- `get-hostname`
- `list-directory`
- `get-date`
- `stat`
- And many more...

## Notes

- Property completion executes the command to discover available properties
- For commands with side effects, use with caution
- Completion is cached during the REPL session for performance
