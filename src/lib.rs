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
//! use madhouse::{execute_commands, prop_allof, Command, CommandWrapper, State, TestContext, scenario};
//! use proptest::prelude::{Just, Strategy};
//! use proptest::strategy::ValueTree;
//! use std::env;
//! use std::sync::Arc;
//!
//! struct IncrementCommand;
//!
//! impl Command for IncrementCommand {
//!     fn check(&self, _state: &State) -> bool { true }
//!     fn apply(&self, state: &mut State) { state.last_mined_block += 1; }
//!     fn label(&self) -> String { "INCREMENT".to_string() }
//!     fn build(ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper> {
//!         Just(CommandWrapper::new(IncrementCommand))
//!     }
//! }
//!
//! let test_context = Arc::new(TestContext::new(vec![]));
//!
//! scenario! [test_context, IncrementCommand];
//! ```

use proptest::prelude::Strategy;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;
use std::time::Instant;

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
    fn build(ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper>
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

/// Creates a strategy that always returns a Vec containing values from all the
/// provided strategies, in the exact order they were passed.
///
/// This is similar to `prop_oneof` but instead of randomly picking strategies,
/// it includes values from all strategies in a Vec.
#[macro_export]
macro_rules! prop_allof {
    ($strat:expr $(,)?) => {
        $strat.prop_map(|val| vec![val])
    };

    ($first:expr, $($rest:expr),+ $(,)?) => {
        {
            let first_strat = $first.prop_map(|val| vec![val]);
            let rest_strat = prop_allof!($($rest),+);

            (first_strat, rest_strat).prop_map(|(mut first_vec, rest_vec)| {
                first_vec.extend(rest_vec);
                first_vec
            })
        }
    };
}

// Function to execute commands and return the executed ones.
pub fn execute_commands<'a>(
    commands: &'a [CommandWrapper],
    state: &mut State,
) -> Vec<&'a CommandWrapper> {
    let mut executed = Vec::with_capacity(commands.len());
    let mut execution_times = Vec::with_capacity(commands.len());

    // ANSI color codes
    let yellow = "\x1b[33m";
    let green = "\x1b[32m";
    let reset = "\x1b[0m";

    for cmd in commands {
        if cmd.command.check(state) {
            let start = Instant::now();
            cmd.command.apply(state);
            let duration = start.elapsed();
            executed.push(cmd);
            execution_times.push(duration);
        }
    }

    println!("Selected:");
    for (i, cmd) in commands.iter().enumerate() {
        println!("{:02}. {}{}{}", i + 1, yellow, cmd.command.label(), reset);
    }

    println!("Executed:");
    for (i, (cmd, time)) in executed.iter().zip(execution_times.iter()).enumerate() {
        println!(
            "{:02}. {}{}{} ({:.2?})",
            i + 1,
            green,
            cmd.command.label(),
            reset,
            time
        );
    }

    executed
}

/// Macro for running stateful tests.
///
/// By default, commands are executed deterministically in the order
/// they are passed. If the `MADHOUSE=1` environment variable is set
/// commands are executed randomly.
///
/// # Arguments
///
/// * `test_context` - The test context to use for creating commands.
/// * `command1, command2, ...` - The actual command objects to test.
#[macro_export]
macro_rules! scenario {
    ($test_context:expr, $($cmd_type:ident),+ $(,)?) => {
        {
            let test_context = $test_context.clone();
            let config = proptest::test_runner::Config {
                cases: 1,
                max_shrink_iters: 0,
                ..Default::default()
            };

            // Use MADHOUSE env var to determine test mode.
            let use_madhouse = env::var("MADHOUSE") == Ok("1".into());

            if use_madhouse {
                proptest::proptest!(config, |(commands in proptest::collection::vec(
                    proptest::prop_oneof![
                        $($cmd_type::build(test_context.clone())),+
                    ],
                    1..16,
                ))| {
                    println!("\n=== New Test Run (MADHOUSE mode) ===\n");
                    let mut state = State::new();
                    execute_commands(&commands, &mut state);
                });
            } else {
                proptest::proptest!(config, |(commands in prop_allof![
                    $($cmd_type::build(test_context.clone())),+
                ])| {
                    println!("\n=== New Test Run (deterministic mode) ===\n");
                    let mut state = State::new();
                    execute_commands(&commands, &mut state);
                });
            }
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

        fn build(_ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper> {
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
    use std::env;
    use std::sync::Arc;
    use std::sync::Mutex;

    // Simple test command that increments block count.
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

        fn build(_ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper> {
            Just(CommandWrapper::new(IncrementCommand))
        }
    }

    #[test]
    fn test_deterministic_mode() {
        env::remove_var("MADHOUSE");

        let ctx = Arc::new(TestContext::new(vec![]));
        scenario![ctx, IncrementCommand];
    }

    #[test]
    fn test_shared_state_persistence() {
        // Test that a shared state accumulates changes across runs.
        let shared_state = Arc::new(Mutex::new(State::new()));

        for i in 0..3 {
            let cmd1 = CommandWrapper::new(IncrementCommand);
            let cmd2 = CommandWrapper::new(IncrementCommand);
            let cmds = vec![cmd1, cmd2];

            let mut state = shared_state.lock().unwrap();
            execute_commands(&cmds, &mut state);

            assert_eq!(state.last_mined_block, (i + 1) * 2);
        }

        // State persisted across all runs.
        assert_eq!(shared_state.lock().unwrap().last_mined_block, 6);
    }
}
