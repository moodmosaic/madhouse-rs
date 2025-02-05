use proptest::prelude::*;

pub struct ExampleState {
  value: i32,
}

impl ExampleState {
  pub fn new() -> Self {
      Self { value: 0 }
  }

  pub fn increment(&mut self) {
      self.value += 1;
  }

  pub fn decrement(&mut self) {
      self.value -= 1;
  }

  pub fn get_value(&self) -> i32 {
      self.value
  }
}

/// A trait that all commands must implement.
pub trait Command {
    fn check(&self, state: &ExampleState) -> bool;
    fn apply(&self, state: &mut ExampleState);
}

/// Increment command.
#[derive(Debug, Clone)]
pub struct IncrementCommand;

impl Command for IncrementCommand {
    fn check(&self, _state: &ExampleState) -> bool {
        true // Always allowed.
    }

    fn apply(&self, state: &mut ExampleState) {
        state.increment();
    }
}

/// Decrement command.
#[derive(Debug, Clone)]
pub struct DecrementCommand;

impl Command for DecrementCommand {
    fn check(&self, state: &ExampleState) -> bool {
        state.get_value() > 0 // Prevents negative values.
    }

    fn apply(&self, state: &mut ExampleState) {
        state.decrement();
    }
}

proptest! {
    #[test]
    fn test_random_commands(commands in proptest::collection::vec(
        prop_oneof![
            Just(Box::new(IncrementCommand) as Box<dyn Command>),
            Just(Box::new(DecrementCommand) as Box<dyn Command>),
        ],
        10 // Run 10 commands per test.
    )) {
        let mut state = ExampleState::new();

        for cmd in commands {
            if cmd.check(&state) {
                cmd.apply(&mut state);
            }
        }

        // Ensure the value is never negative.
        assert!(state.get_value() >= 0);
    }
}
