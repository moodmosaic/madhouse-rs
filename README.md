# madhouse-rs

Model-based Rust state machine testing.

## Overview

Tests state machines via sequences of command objects. Each command:
1. Checks preconditions via check()
2. Mutates state via apply()
3. Verifies assertions

### Command flow

```
                   +-------+
                   | State |
                   +-------+
                       ^
                       |
  +---------+     +----+----+     +-----------+
  | Command | --> | check() | --> |  apply()  |
  +---------+     +---------+     | [asserts] |
       ^                          +-----------+
       |
  +----------+
  | Strategy |
  +----------+
```

## Features

- Trait-based command design
- Self-validating commands
- Timing information
- Test case shrinking

## Usage

```rust
use madhouse::prelude::*;
use proptest::prelude::*;
use std::sync::Arc;

#[derive(Debug, Default)]
struct Counter {
    value: u32,
}
impl State for Counter {}

#[derive(Clone, Debug, Default)]
struct Ctx {}
impl TestContext for Ctx {}

struct Increment {
    amount: u32,
}
impl Command<Counter, Ctx> for Increment {
    fn check(&self, _: &Counter) -> bool {
        true
    }

    fn apply(&self, state: &mut Counter) {
        state.value += self.amount;
        assert!(
            state.value <= 100,
            "Counter value exceeded maximum allowed: {}",
            state.value
        );
    }

    fn label(&self) -> String {
        format!("INCREMENT({})", self.amount)
    }

    fn build(_: Arc<Ctx>)
        -> impl Strategy<Value = CommandWrapper<Counter, Ctx>>
    {
        (1..=50u32).prop_map(|amount| {
            CommandWrapper::new(Increment { amount })
        })
    }
}

struct Decrement {
    amount: u32,
}
impl Command<Counter, Ctx> for Decrement {
    fn check(&self, state: &Counter) -> bool {
        state.value >= self.amount
    }

    fn apply(&self, state: &mut Counter) {
        state.value -= self.amount;
    }

    fn label(&self) -> String {
        format!("DECREMENT({})", self.amount)
    }

    fn build(_: Arc<Ctx>)
        -> impl Strategy<Value = CommandWrapper<Counter, Ctx>>
    {
        (1..=10u32).prop_map(|amount| {
            CommandWrapper::new(Decrement { amount })
        })
    }
}

#[test]
fn test_counter() {
    let ctx = Arc::new(Ctx::default());
    scenario![ctx, Increment, Decrement, (Increment { amount: 42 })];
}
```

## Testing Modes

- **Normal**: Commands run in specified order but proptest strategies will generate different values across runs unless using a fixed seed
- **Random**: Commands chosen pseudorandomly (set `MADHOUSE=1`)
- **Shrinking**: To shrink test cases, set `PROPTEST_MAX_SHRINK_ITERS`

## Examples

### Basic Usage

Run tests on your own projects:

```bash
# Normal mode
cargo test

# Random mode
MADHOUSE=1 cargo test

# With shrinking
MADHOUSE=1 PROPTEST_MAX_SHRINK_ITERS=50 cargo test
```

### Interactive Playground

Try the [playground script](scripts/playground.sh) for a hands-on demo.

```bash
# Normal mode
./scripts/playground.sh

# Random mode
MADHOUSE=1 ./scripts/playground.sh

# With shrinking
MADHOUSE=1 PROPTEST_MAX_SHRINK_ITERS=50 ./scripts/playground.sh
```

The playground script:
- Creates a sample counter project with increment/decrement commands
- Demonstrates test failure detection and preservation
- Shows how shrinking works to find minimal failing test cases

## License

GPL-3.0

Copyright (C) 2025 Stacks Open Internet Foundation. <https://stacks.org/>
