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

/// Macro for running stateful tests.
#[macro_export]
macro_rules! madhouse {
    ($test_context:expr, [ $( $command:ident ),* ], $min:expr, $max:expr) => {
        let config = proptest::test_runner::Config { cases: 1, ..Default::default() };

        proptest::proptest!(config, |(commands in proptest::collection::vec(
            proptest::prop_oneof![ $( $command::build(&$test_context), )* ],
            $min..$max,
        ))| {
            let mut state = $crate::State::new();
            let mut executed = Vec::with_capacity(commands.len());

            for cmd in &commands {
                if cmd.command.check(&state) {
                    cmd.command.apply(&mut state);
                    executed.push(cmd);
                }
            }

            println!("Commands: {:?}", commands);
            println!("Executed: {:?}", executed);
        });
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
