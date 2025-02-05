use proptest::prelude::*;
use rust_command_testing::{Command, DecrementCommand, ExampleState, IncrementCommand};

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
