# Stargate Shell Scripting Tests

This directory contains example scripts demonstrating the stargate-shell scripting language features.

## Running the tests

Make sure stargate-shell is built first:
```bash
cargo build --bin stargate-shell
```

Then run any script:
```bash
./target/debug/stargate-shell examples/scripting_tests/variables.sg
```

Or make them executable and run directly:
```bash
chmod +x examples/scripting_tests/*.sg
./examples/scripting_tests/variables.sg
```

## Test files

- `variables.sg` - Variable declarations and arithmetic operations
- `conditionals.sg` - If/else conditional logic
- `functions.sg` - Function definitions, calls, and recursion
- `stargate_commands.sg` - Executing stargate commands from scripts
- `complex_logic.sg` - Combining functions and conditionals

## Language Features

The scripting language supports:

- **Variables**: `let x = 5;`
- **Arithmetic**: `+`, `-`, `*`, `/`
- **Comparisons**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Boolean operators**: `&&`, `||`
- **Conditionals**: `if condition { ... } else { ... }`
- **Functions**: `fn name(params) { ... return value; }`
- **Command execution**: `exec "command";`
- **Print**: `print value;`
- **Comments**: `# comment`
