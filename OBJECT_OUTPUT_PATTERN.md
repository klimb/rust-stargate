# Object Output Pattern for Commands

This document outlines the standard pattern for adding `-o/--obj` object output support (producing JSON) to commands.

## Pattern Overview

All commands should support `-o/--obj` flag for object output (JSON) that works with `slice-object` and `dice-object` utilities.

## Implementation Checklist

### 1. Add Dependencies to Cargo.toml

```toml
[dependencies]
clap = { workspace = true }
uucore = { workspace = true }
fluent = { workspace = true }
serde_json = { workspace = true }  # Add this
```

### 2. Add Imports to Source File

```rust
use uucore::json_output::{self, JsonOutputOptions};
use serde_json::json;
```

### 3. Update uu_app() Function

Add object output args to command:

```rust
pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        // ... existing configuration ...
        ;
    
    json_output::add_json_args(cmd)  // Add this line
}
```

### 4. Update uumain() Function

Parse options and add object output logic:

```rust
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let opts = JsonOutputOptions::from_matches(&matches);  // Add this
    
    // ... existing logic to get data ...
    
    if opts.json_output {
        let output = json!({
            "field1": value1,
            "field2": value2,
            // Include relevant metadata
        });
        json_output::output(opts, output, || Ok(()))?;
    } else {
        // ... existing output logic ...
    }
    Ok(())
}
```

## Example: pwd Command

### Before
```rust
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let cwd = physical_path()?;
    println_verbatim(cwd)?;
    Ok(())
}
```

### After
```rust
use uucore::json_output::{self, JsonOutputOptions};
use serde_json::json;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let opts = JsonOutputOptions::from_matches(&matches);
    let cwd = physical_path()?;
    
    if opts.json_output {
        let path_str = cwd.to_string_lossy().to_string();
        let output = json!({
            "path": path_str,
            "absolute": cwd.is_absolute(),
        });
        json_output::output(opts, output, || Ok(()))?;
    } else {
        println_verbatim(cwd)?;
    }
    Ok(())
}

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        // ... configuration ...
        ;
    json_output::add_json_args(cmd)
}
```

## Testing

```bash
# Test basic object output
command -o

# Test with dice-object (column filtering)
command -o | dice-object -f field1 --pretty

# Test with slice-object (row extraction)
command -o | slice-object -f results | dice-object -f name
```

## Commands with Object Output Support

- [x] pwd
- [x] whoami  
- [x] arch
- [x] basename
- [x] dirname
- [x] find-text
- [x] list-directory
- [x] get_fqdn
- [ ] cat
- [ ] cut
- [ ] date
- [ ] du
- [ ] df
- [ ] head
- [ ] tail
- [ ] wc
- [ ] sort
- [ ] uniq
- [ ] ... (add more as implemented)

## Field Naming Guidelines

Use snake_case for field names:
- `file_name`, `file_size`, `modified_time`
- Avoid abbreviations unless standard (e.g., `uid`, `gid`)
- Be consistent across similar commands

## Metadata to Include

Consider including:
- Primary output data
- Counts/statistics
- Flags/modes used
- Timestamps where relevant
- File paths (absolute when possible)
