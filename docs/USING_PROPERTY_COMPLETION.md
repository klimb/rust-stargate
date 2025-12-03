# Using Property Completion - Step by Step Guide

## What You're Trying to Do

You want to use tab completion to discover and access properties from JSON objects returned by stargate commands.

## The Problem You're Experiencing

When you type `get-hostname.` or `(get-directory).`, nothing seems to happen. This is because **you need to press the TAB key** after typing the dot.

## Step-by-Step Instructions

### 1. Start the Interactive Shell

```bash
$ cd /home/dvk/src/rust-stargate
$ ./target/debug/stargate-shell
```

You should see:
```
Stargate Shell 0.4.0
A Unix-like shell for chaining stargate commands with JSON pipes
Type 'help' for usage, 'exit' to quit
Use Tab for command completion

stargate> 
```

### 2. Type a Command Followed by a Dot

```
stargate> get-hostname.
```

**Don't press Enter!** The cursor should be right after the dot.

### 3. Press the TAB Key

Press the `Tab` key on your keyboard (usually above Caps Lock).

You should see:
```
stargate> get-hostname.
flags      hostname
```

### 4. Continue Typing or Tab Again

- Type `h` and press TAB again to complete to `hostname`
- Or just type the full property name

### 5. Press Enter to Execute

```
stargate> (get-hostname).hostname
theone
```

## Alternative: With Parentheses

You can also use parentheses explicitly:

```
stargate> (get-hostname).     ← Press TAB here
flags      hostname
```

## Using in Scripts (Script Mode)

For actual scripting (not just property completion), use the `script` command:

```
stargate> script let x = (get-hostname).hostname;
stargate> script print x;
theone
```

**Note**: Each `script` command runs independently, so variables don't persist between commands in the REPL. For persistent scripts, use script files:

```bash
$ cat > my_script.sg << 'EOF'
#!/usr/bin/env stargate-shell
let host = (get-hostname).hostname;
print "Hostname: {host}";
EOF

$ chmod +x my_script.sg
$ ./target/debug/stargate-shell my_script.sg
Hostname: theone
```

## Common Commands to Try

### get-hostname
```
stargate> get-hostname.     ← TAB
flags      hostname
```

Properties:
- `flags` - Object with command flags
- `hostname` - The actual hostname string

### list-directory
```
stargate> list-directory.   ← TAB
entries       recursive      count
```

Properties:
- `entries` - Array of directory entries
- `recursive` - Boolean indicating if recursive
- `count` - Number of entries

### Accessing Array Elements
```
stargate> (list-directory).entries[0].   ← TAB
gid    inode    modified    name    nlink    path    permissions    size    type    uid
```

## Troubleshooting

### "Nothing happens when I type ."
- **Solution**: You must press the TAB key, not just type the dot

### "I get 'Script error' in the REPL"
- **Solution**: In REPL `script` mode, each command is independent. Use script files for multi-statement programs.

### "Completion shows but doesn't insert parentheses"
- **Current behavior**: The completion logic detects when parentheses are needed, but you may need to manually type them for now
- **Best practice**: Always use parentheses: `(command).property`

### "How do I access nested properties?"
```bash
# In a script file:
let first_entry = (list-directory).entries[0];
let first_name = first_entry.name;
print first_name;

# Or in one line:
let first_name = (list-directory).entries[0].name;
print first_name;
```

## Summary

1. **Start shell**: `./target/debug/stargate-shell`
2. **Type command with dot**: `get-hostname.`
3. **Press TAB**: ← This is the key step!
4. **See completions**: `flags  hostname`
5. **Select and execute**

The feature is working - you just need to **press TAB** to activate completion!
