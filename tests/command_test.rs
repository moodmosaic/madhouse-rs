use proptest::prelude::*;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;

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
    fn name(&self) -> &'static str; // Added for debugging.
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

    fn name(&self) -> &'static str {
        "IncrementCommand"
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

    fn name(&self) -> &'static str {
        "DecrementCommand"
    }
}

/// Wrapper to make `dyn Command` clonable and debuggable.
#[derive(Clone)]
struct CommandWrapper {
    command: Arc<dyn Command>,
}

impl CommandWrapper {
    fn new<C: Command + 'static>(cmd: C) -> Self {
        Self {
            command: Arc::new(cmd),
        }
    }
}

// Manually implement Debug for `CommandWrapper`.
impl Debug for CommandWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.command.name()) // Print command name.
    }
}

proptest! {
    #[test]
    fn test_random_commands(commands in proptest::collection::vec(
        prop_oneof![
            Just(CommandWrapper::new(IncrementCommand)),
            Just(CommandWrapper::new(DecrementCommand)),
        ],
        10 // Run 10 commands per test.
    )) {
        let mut state = ExampleState::new();

        for cmd in &commands {
            if cmd.command.check(&state) {
                cmd.command.apply(&mut state);
            }
        }

        // Debugging output.
        println!("Executed commands: {:?}", commands);

        // Ensure the value is never negative.
        assert!(state.get_value() >= 0);
    }
}
