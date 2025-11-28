# Stargate Shell Scripting 

This directory contains example scripts demonstrating the stargate-shell scripting language features.

## Running the tests

add stargate-shell to your path, so /usr/bin/env recognizes it (zsh example that asssumes /src/rust-stargate is where you have stargate):

``` 
% echo 'export PATH="$HOME/src/rust-stargate/target/debug:$PATH"' >> ~/.zshrc
source ~/.zshrc

```

make them executable and run them as any other shell script (such as .sh):
```
chmod +x examples/scripting_tests/*.sg
```

and run a stargate script:
```
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
