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

## Usage

```rust
use madhouse::*;
use proptest::prelude::*;
use std::env;
use std::sync::Arc;

// State + Context
#[derive(Debug, Default)]
struct Counter {
    value: u32,
    max: u32,
}
impl State for Counter {}

#[derive(Debug, Clone, Default)]
struct Ctx {}
impl TestContext for Ctx {}

// Commands
struct Inc {
    amount: u32,
}
impl Command<Counter, Ctx> for Inc {
    fn check(&self, s: &Counter) -> bool {
        s.value + self.amount <= s.max
    }
    fn apply(&self, s: &mut Counter) {
        s.value += self.amount;
    }
    fn label(&self) -> String {
        format!("INC({})", self.amount)
    }
    fn build(_: Arc<Ctx>) -> impl Strategy<Value = CommandWrapper<Counter, Ctx>> {
        (1..=5u32).prop_map(|n| CommandWrapper::new(Inc { amount: n }))
    }
}

struct Reset;
impl Command<Counter, Ctx> for Reset {
    fn check(&self, s: &Counter) -> bool {
        s.value > 0
    }
    fn apply(&self, s: &mut Counter) {
        s.value = 0;
    }
    fn label(&self) -> String {
        "RESET".to_string()
    }
    fn build(_: Arc<Ctx>) -> impl Strategy<Value = CommandWrapper<Counter, Ctx>> {
        Just(CommandWrapper::new(Reset))
    }
}

fn main() {
    let ctx = Arc::new(Ctx::default());
    scenario![ctx, Inc, Reset, (Inc { amount: 42 })];
}
```

## Testing Modes

- **Normal**: Commands run in specified order but proptest strategies will generate different values across runs unless using a fixed seed
- **Random**: Commands chosen pseudorandomly (set `MADHOUSE=1`)
- **Shrinking**: To shrink test cases, set `PROPTEST_MAX_SHRINK_ITERS`

## Example

Run tests:
```bash
# Normal mode
cargo test

# Random mode
MADHOUSE=1 cargo test

# With shrinking
MADHOUSE=1 PROPTEST_MAX_SHRINK_ITERS=100 cargo test
```

## Features

- Trait-based command design
- Self-validating commands
- Timing information
- Test case shrinking

## License

GPL-3.0

Copyright (C) 2025 Stacks Open Internet Foundation. <https://stacks.org/>
