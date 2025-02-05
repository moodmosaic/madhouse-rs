use proptest::prelude::*;
use crate::ExampleState;

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
