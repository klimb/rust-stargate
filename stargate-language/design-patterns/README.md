# Design Patterns in Stargate Language

This directory contains examples of common design patterns implemented in the Stargate shell language.

## Builder Pattern - Pizza Builder

The Pizza Builder demonstrates the **Builder Design Pattern** with fluent method chaining using the `this` keyword.

### Key Language Features Demonstrated:

1. **`this` keyword** - Returns the current instance from methods for chaining
2. **Method chaining** - Fluent interface pattern 
3. **Classes with methods** - Object-oriented programming
4. **List operations** - Dynamic topping management

### Running the Example:

```bash
./target/debug/stargate-shell stargate-language/scripts/stargate-lang/design-patterns/pizza_builder.sg
```

### New Language Features Added:

- **`this` keyword support** - Added to enable method chaining in the builder pattern
  - Returns the current instance when used inside a method
  - Enables fluent interfaces like: `builder.method1().method2().method3()`
  
This enhancement required updates to:
- AST (Expression::This variant)
- Parser (recognizing "this" as a keyword)
- Interpreter (tracking current instance context)
- Expression evaluator (returning current instance for Expression::This)
