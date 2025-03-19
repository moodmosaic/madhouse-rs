//! # madhouse-rs
//!
//! Model-based stateful testing for the stacks-node.
//!
//! This library provides infra for writing property-based tests
//! that exercise the stacks-node through sequences of commands.
//!
//! ## Overview
//!
//! Stateful systems often have complex behaviors:
//! - Many hardcoded test sequences are needed.
//! - Timing-dependent behavior is hard to test systematically.
//! - Manual test case construction is slow.
//! - Properties span multiple operations.
//!
//! This framework implements state machine testing:
//!
//! ```text
//!                    +-------+
//!                    | State |
//!                    +-------+
//!                        ^
//!                        |
//!   +---------+     +----+----+     +-----------+
//!   | Command | --> | check() | --> |  apply()  |
//!   +---------+     +---------+     | [asserts] |
//!                                   +-----------+
//!        ^                                |
//!        |                                v
//!   +----------+                      +--------+
//!   | Strategy |                      | State' |
//!   +----------+                      +--------+
//! ```
//!
//! Each command:
//! 1. Has a strategy for generation.
//! 2. Checks preconditions before execution.
//! 3. Applies state changes.
//! 4. Asserts correctness.
//!
//! ## Benefits
//!
//! - Trait-based design encapsulates behavior.
//! - Commands are autonomous and self-validating.
//! - Enables both property-based and deterministic testing.
//!
//! ## Example
//!
//! ```rust
//! use madhouse::{Command, CommandWrapper, State, TestContext, madhouse};
//! use proptest::prelude::{Just, Strategy};
//! use proptest::strategy::ValueTree;
//!
//! struct IncrementCommand;
//!
//! impl Command for IncrementCommand {
//!     fn check(&self, _state: &State) -> bool { true }
//!     fn apply(&self, state: &mut State) { state.last_mined_block += 1; }
//!     fn label(&self) -> String { "INCREMENT".to_string() }
//!     fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
//!         Just(CommandWrapper::new(IncrementCommand))
//!     }
//! }
//!
//! let test_context = TestContext::new(vec![]);
//!
//! madhouse!(test_context, [IncrementCommand], 1, 5);
//! ```

use proptest::prelude::Strategy;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;

/// Miner seed type.
pub type MinerSeed = Vec<u8>;

/// Test context holds configuration for test generation.
#[derive(Clone, Debug)]
pub struct TestContext {
    pub miner_seeds: Vec<MinerSeed>,
}

impl TestContext {
    pub fn new(miner_seeds: Vec<MinerSeed>) -> Self {
        Self { miner_seeds }
    }
}

/// State tracked during test execution.
#[derive(Default, Debug)]
pub struct State {
    pub running_miners: HashSet<MinerSeed>,
    pub last_mined_block: u64,
    pub block_commits: HashMap<u64, HashSet<MinerSeed>>,
    pub block_leaders: HashMap<u64, MinerSeed>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_miner_running(&self, seed: &MinerSeed) -> bool {
        self.running_miners.contains(seed)
    }

    pub fn next_block_height(&self) -> u64 {
        self.last_mined_block + 1
    }

    pub fn start_miner(&mut self, miner_seed: &[u8]) {
        self.running_miners.insert(miner_seed.to_vec());
    }

    pub fn add_block_commit(&mut self, height: u64, miner_seed: &[u8]) {
        self.block_commits
            .entry(height)
            .or_default()
            .insert(miner_seed.to_vec());
    }

    pub fn add_sortition_block_leader(&mut self, height: u64, miner_seed: &[u8]) {
        if self.block_leaders.contains_key(&height) {
            panic!("Sortition already happened for height {}.", height);
        }
        self.block_leaders.insert(height, miner_seed.to_vec());
    }
}

/// Trait for commands in the stateful testing framework.
pub trait Command {
    fn check(&self, state: &State) -> bool;
    fn apply(&self, state: &mut State);
    fn label(&self) -> String;
    fn build(ctx: &TestContext) -> impl Strategy<Value = CommandWrapper>
    where
        Self: Sized;
}

/// Wrapper for command trait objects.
#[derive(Clone)]
pub struct CommandWrapper {
    pub command: Arc<dyn Command>,
}

impl CommandWrapper {
    pub fn new<C: Command + 'static>(cmd: C) -> Self {
        Self {
            command: Arc::new(cmd),
        }
    }
}

impl Debug for CommandWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.command.label())
    }
}

// Function to execute commands and return the executed ones
pub fn execute_commands<'a>(
    commands: &'a [CommandWrapper],
    state: &mut State,
) -> Vec<&'a CommandWrapper> {
    let mut executed = Vec::with_capacity(commands.len());

    for cmd in commands {
        if cmd.command.check(state) {
            cmd.command.apply(state);
            executed.push(cmd);
        }
    }

    executed
}

/// Macro for running stateful tests.
///
/// By default, commands are executed deterministically in the order they are passed.
/// If the MADHOUSE=1 environment variable is set, commands are executed randomly.
///
/// # Arguments
///
/// * `test_context` - The test context to use for generating commands.
/// * `[command1, command2, ...]` - The command types to use for generating commands.
/// * `min` - The minimum number of commands to generate.
/// * `max` - The maximum number of commands to generate.
#[macro_export]
macro_rules! madhouse {
    ($test_context:expr, [ $( $command:ident ),* ], $min:expr, $max:expr) => {
        let use_random = std::env::var("MADHOUSE").map(|v| v == "1").unwrap_or(false);

        if use_random {
            // Random execution mode with shared state
            let shared_state = std::sync::Arc::new(std::sync::Mutex::new($crate::State::new()));
            let config = proptest::test_runner::Config {
                cases: 256, // Default to 256 cases for thorough testing
                ..Default::default()
            };

            proptest::proptest!(config, |(commands in proptest::collection::vec(
                proptest::prop_oneof![ $( $command::build(&$test_context), )* ],
                $min..$max,
            ))| {
                // Get mutable access to the shared state
                let mut state = shared_state.lock().unwrap();
                let executed = $crate::execute_commands(&commands, &mut *state);

                println!("Random Commands: {:?}", commands);
                println!("Executed: {:?}", executed);

                // Release the lock
                drop(state);
            });
        } else {
            // Deterministic execution mode
            let mut test_runner = proptest::test_runner::TestRunner::default();
            let mut commands = Vec::new();

            // Generate commands in sequence, cycling through command types
            let command_types = vec![ $( stringify!($command), )* ];
            let mut type_index = 0;
            let mut attempts = 0;

            while commands.len() < $min && attempts < 100 {
                let command_name = command_types[type_index];

                // Generate the command based on its type
                $(
                    if command_name == stringify!($command) {
                        let strategy = $command::build(&$test_context);
                        if let Ok(value) = strategy.new_tree(&mut test_runner).map(|v| v.current()) {
                            if commands.len() < $max {
                                commands.push(value);
                            }
                        }
                    }
                )*

                // Move to next command type
                type_index = (type_index + 1) % command_types.len();
                attempts += 1;
            }

            let mut state = $crate::State::new();
            let executed = $crate::execute_commands(&commands, &mut state);

            println!("Deterministic Commands: {:?}", commands);
            println!("Executed: {:?}", executed);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::Just;

    struct TestCommand {
        value: u32,
    }

    impl Command for TestCommand {
        fn check(&self, _state: &State) -> bool {
            true
        }
        fn apply(&self, state: &mut State) {
            state.last_mined_block += self.value as u64;
        }
        fn label(&self) -> String {
            format!("TEST({})", self.value)
        }
        fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
            Just(CommandWrapper::new(TestCommand { value: 1 }))
        }
    }

    #[test]
    fn test_command_wrapper() {
        let cmd = TestCommand { value: 42 };
        let wrapper = CommandWrapper::new(cmd);

        let mut state = State::new();
        assert!(wrapper.command.check(&state));
        wrapper.command.apply(&mut state);
        assert_eq!(state.last_mined_block, 42);
        assert_eq!(format!("{:?}", wrapper), "TEST(42)");
    }

    #[test]
    fn test_state() {
        let mut state = State::new();
        assert_eq!(state.next_block_height(), 1);

        let miner_seed = vec![1, 2, 3];
        state.start_miner(&miner_seed);
        assert!(state.is_miner_running(&miner_seed));

        state.add_block_commit(1, &miner_seed);
        assert!(state.block_commits.contains_key(&1));
        assert!(state.block_commits[&1].contains(&miner_seed));

        state.add_sortition_block_leader(1, &miner_seed);
        assert!(state.block_leaders.contains_key(&1));
        assert_eq!(state.block_leaders[&1], miner_seed);
    }

    #[test]
    fn test_test_context() {
        let miner_seeds = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let ctx = TestContext::new(miner_seeds.clone());
        assert_eq!(ctx.miner_seeds, miner_seeds);
    }
}

#[cfg(test)]
mod macro_tests {
    use super::*;
    use proptest::prelude::Just;
    use proptest::strategy::ValueTree;
    use std::env;
    use std::sync::Arc;
    use std::sync::Mutex;

    // Simple test command that increments block count
    struct IncrementCommand;

    impl Command for IncrementCommand {
        fn check(&self, _state: &State) -> bool {
            true
        }
        fn apply(&self, state: &mut State) {
            state.last_mined_block += 1;
        }
        fn label(&self) -> String {
            "INCREMENT".to_string()
        }
        fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
            Just(CommandWrapper::new(IncrementCommand))
        }
    }

    #[test]
    fn test_deterministic_mode() {
        // Ensure MADHOUSE is not set
        env::remove_var("MADHOUSE");

        let ctx = TestContext::new(vec![]);
        madhouse!(ctx, [IncrementCommand], 3, 5);
        // This test simply verifies the macro runs without errors in deterministic mode
    }

    #[test]
    fn test_shared_state_persistence() {
        // Test that a shared state properly accumulates changes across runs
        let shared_state = Arc::new(Mutex::new(State::new()));

        // Multiple runs, using the same state
        for i in 0..3 {
            // Create commands for this run
            let cmd1 = CommandWrapper::new(IncrementCommand);
            let cmd2 = CommandWrapper::new(IncrementCommand);
            let commands = vec![cmd1, cmd2]; // Two commands per run

            // Execute with the shared state
            let mut state = shared_state.lock().unwrap();
            execute_commands(&commands, &mut state);

            // Verify cumulative changes
            assert_eq!(state.last_mined_block, (i + 1) * 2);
        }

        // Final verification - state persisted across all runs
        assert_eq!(shared_state.lock().unwrap().last_mined_block, 6);
    }
}
