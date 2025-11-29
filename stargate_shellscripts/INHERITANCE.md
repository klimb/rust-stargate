# Class Inheritance in Stargate Shell

## Overview
Stargate shell supports **single inheritance** for classes, allowing child classes to inherit fields from parent classes with full support for multi-level inheritance chains.

## Simple Example

```stargate
# Base class
class Animal {
    let name = "Unknown";
    let sound = "...";
    let age = 0;
}

# Dog inherits from Animal
class Dog extends Animal {
    let sound = "Woof";      # Override parent's sound
    let breed = "Mixed";     # New field specific to dogs
}

# Cat inherits from Animal
class Cat extends Animal {
    let sound = "Meow";      # Override parent's sound
    let indoor = true;       # New field specific to cats
}

# Create instances
let dog = new Dog;
let cat = new Cat;

# Dog has: name="Unknown", sound="Woof", age=0, breed="Mixed"
# Cat has: name="Unknown", sound="Meow", age=0, indoor=true
```

## Features

### 1. Field Inheritance
Child classes automatically receive all fields from their parent class.

```stargate
class Animal {
    let name = "Unknown";
}

class Dog extends Animal {
    let breed = "Mixed";
}

let dog = new Dog;
# dog has both: name and breed
```

### 2. Field Overriding
Child classes can change the default values of inherited fields.

```stargate
class Animal {
    let sound = "...";
}

class Dog extends Animal {
    let sound = "Woof";  # Replaces "..." with "Woof"
}
```

### 3. Multi-Level Inheritance
Inheritance chains can be multiple levels deep.

```stargate
class Animal {
    let age = 0;
}

class Dog extends Animal {
    let breed = "Mixed";
}

class Puppy extends Dog {
    let age = 1;        # Override grandparent's age
    let playful = true;
}

let puppy = new Puppy;
# puppy has: age=1, breed="Mixed", playful=true
```

## Implementation Details

### Parser
- Added `parent: Option<String>` field to `ClassDef` AST node
- Parser recognizes `extends ParentClassName` syntax after class name
- Parent name is optional (None for base classes)

### Interpreter
- Uses recursive field collection: `collect_inherited_fields()`
- Traverses inheritance chain from base to derived
- Fields are collected in order: Grandparent → Parent → Child
- Later fields override earlier fields with same name

### AST Structure
```rust
Statement::ClassDef {
    name: String,
    parent: Option<String>,
    fields: Vec<(String, Expression)>,
    methods: Vec<(String, Vec<String>, Vec<Statement>)>
}
```

## Examples

See the following demo scripts:
- `inheritance_demo.sg` - Basic single and multi-level inheritance
- `inheritance_advanced.sg` - Complex inheritance trees with multiple branches
- `classes_demo.sg` - Basic class usage without inheritance

## Limitations

1. **Single Inheritance Only**: Each class can have at most one parent
2. **No Multiple Inheritance**: Cannot inherit from multiple classes
3. **Methods Not Callable**: Method inheritance structure exists but methods cannot be invoked yet
4. **No Constructor Parameters**: All field values come from default expressions
5. **No `super` Keyword**: Cannot explicitly call parent class methods
6. **No `self`/`this` Reference**: Methods cannot reference instance fields

## Future Enhancements

- [ ] Method calls with `instance.method()` syntax
- [ ] `self` reference for accessing instance fields in methods
- [ ] Constructor parameters for initializing fields
- [ ] `super` keyword for calling parent methods
- [ ] Multiple inheritance or traits/interfaces
- [ ] Static fields and methods
- [ ] Private/public field visibility

## Testing

All 23 test scripts pass, including:
- 21 previous feature tests (logical operators, type checking, etc.)
- 1 basic class demo
- 2 inheritance demos (basic and advanced)

## Performance Notes

- Recursive inheritance collection happens at instance creation time
- Each field default expression is evaluated during instantiation
- O(n) complexity where n = total number of fields in inheritance chain
- No runtime overhead for field access (direct HashMap lookup)
