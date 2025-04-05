//! # madhouse-rs
//!
//! Model-based stateful testing for the stacks-node.
//!
//! This library provides infrastructure for writing property-based tests
//! that exercise the stacks-node through sequences of commands. It supports
//! both deterministic and random testing approaches.
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
//! - Supports automatic test case shrinking for easier debugging.
//!
//! ## Execution Modes
//!
//! - **Deterministic mode**: Commands are executed in the order specified
//!   (default mode)
//! - **Random mode**: Commands are randomly selected using proptest
//!   (activated by setting `MADHOUSE=1` environment variable)
//!
//! ## Example
//!
//! ```rust
//! use madhouse::{
//!     execute_commands, prop_allof, Command, CommandWrapper, State,
//!     TestContext, scenario
//! };
//! use proptest::prelude::{Just, Strategy};
//! use proptest::strategy::ValueTree;
//! use std::env;
//! use std::sync::Arc;
//!
//! // Define a simple increment command.
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
//! // Set up test context.
//! let test_context = Arc::new(TestContext::new(vec![]));
//!
//! // Run the test scenario.
//! scenario! [test_context, IncrementCommand];
//!
//! // Manual execution.
//! let mut state = State::new();
//! let commands = vec![CommandWrapper::new(IncrementCommand)];
//! let executed = execute_commands(&commands, &mut state);
//! assert_eq!(state.last_mined_block, 1);
//! ```

use proptest::prelude::Strategy;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;
use std::time::Instant;

/// Miner seed type. Used to uniquely identify miners in the network.
pub type MinerSeed = Vec<u8>;

/// Test context holds configuration for test generation.
/// This provides shared context data that commands can use during test
/// generation.
#[derive(Clone, Debug)]
pub struct TestContext {
    /// Seeds for miners available to use in tests.
    /// Each seed uniquely identifies a miner.
    pub miner_seeds: Vec<MinerSeed>,
}

impl TestContext {
    /// Creates a new test context with the provided miner seeds.
    ///
    /// # Arguments
    ///
    /// * `miner_seeds` - List of miner seeds available for use in tests.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::TestContext;
    ///
    /// let seeds = vec![vec![1, 2, 3], vec![4, 5, 6]];
    /// let ctx = TestContext::new(seeds.clone());
    /// assert_eq!(ctx.miner_seeds, seeds);
    /// ```
    pub fn new(miner_seeds: Vec<MinerSeed>) -> Self {
        Self { miner_seeds }
    }
}

/// State tracked during test execution.
/// Represents the current state of the system being tested.
/// Commands check against and modify this state.
#[derive(Default, Debug)]
pub struct State {
    /// Set of miners that are currently running.
    pub running_miners: HashSet<MinerSeed>,

    /// The height of the most recently mined block.
    pub last_mined_block: u64,

    /// Map from block heights to the set of miners that committed to mine it.
    pub block_commits: HashMap<u64, HashSet<MinerSeed>>,

    /// Map from block heights to the miner that won the sortition.
    pub block_leaders: HashMap<u64, MinerSeed>,
}

impl State {
    /// Creates a new, empty state.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let state = State::new();
    /// assert_eq!(state.last_mined_block, 0);
    /// assert!(state.running_miners.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a miner with the given seed is running.
    ///
    /// # Arguments
    ///
    /// * `seed` - The seed of the miner to check.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let mut state = State::new();
    /// let miner_seed = vec![1, 2, 3];
    ///
    /// assert!(!state.is_miner_running(&miner_seed));
    ///
    /// state.start_miner(&miner_seed);
    /// assert!(state.is_miner_running(&miner_seed));
    /// ```
    pub fn is_miner_running(&self, seed: &MinerSeed) -> bool {
        self.running_miners.contains(seed)
    }

    /// Returns the height of the next block to be mined.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let mut state = State::new();
    /// assert_eq!(state.next_block_height(), 1);
    ///
    /// // After mining a block
    /// state.last_mined_block = 5;
    /// assert_eq!(state.next_block_height(), 6);
    /// ```
    pub fn next_block_height(&self) -> u64 {
        self.last_mined_block + 1
    }

    /// Adds a miner to the set of running miners.
    ///
    /// # Arguments
    ///
    /// * `miner_seed` - Seed of the miner to start.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let mut state = State::new();
    /// let miner_seed = vec![1, 2, 3];
    ///
    /// state.start_miner(&miner_seed);
    /// assert!(state.is_miner_running(&miner_seed));
    ///
    /// // Starting the same miner again is idempotent
    /// state.start_miner(&miner_seed);
    /// assert!(state.is_miner_running(&miner_seed));
    /// ```
    pub fn start_miner(&mut self, miner_seed: &[u8]) {
        self.running_miners.insert(miner_seed.to_vec());
    }

    /// Records a block commitment from a miner at the specified height.
    ///
    /// # Arguments
    ///
    /// * `height` - Block height the miner is committing to.
    /// * `miner_seed` - Seed of the committing miner.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let mut state = State::new();
    /// let miner_seed = vec![1, 2, 3];
    /// let height = 1;
    ///
    /// state.add_block_commit(height, &miner_seed);
    /// assert!(state.block_commits.contains_key(&height));
    /// assert!(state.block_commits[&height].contains(&miner_seed));
    /// ```
    pub fn add_block_commit(&mut self, height: u64, miner_seed: &[u8]) {
        self.block_commits
            .entry(height)
            .or_default()
            .insert(miner_seed.to_vec());
    }

    /// Records the winner of a sortition for a specific block height.
    ///
    /// # Arguments
    ///
    /// * `height` - Block height for the sortition.
    /// * `miner_seed` - Seed of the winning miner.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if successful.
    /// * `Err` with an error message if sortition already happened.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::State;
    ///
    /// let mut state = State::new();
    /// let miner_seed = vec![1, 2, 3];
    /// let height = 1;
    ///
    /// // First sortition succeeds.
    /// assert!(state.add_sortition_block_leader(height, &miner_seed).is_ok());
    /// assert!(state.block_leaders.contains_key(&height));
    /// assert_eq!(state.block_leaders[&height], miner_seed);
    ///
    /// // Attempting to store a second sortition winner for the same height fails.
    /// let result = state.add_sortition_block_leader(height, &miner_seed);
    /// assert!(result.is_err());
    /// ```
    pub fn add_sortition_block_leader(
        &mut self,
        height: u64,
        miner_seed: &[u8],
    ) -> Result<(), String> {
        if self.block_leaders.contains_key(&height) {
            return Err(format!("Sortition already happened for height {}.", height));
        }
        self.block_leaders.insert(height, miner_seed.to_vec());
        Ok(())
    }
}

/// Trait for commands in the stateful testing framework.
/// Each command represents an action that can be performed in the system.
/// Commands are responsible for:
/// - Checking if they can be applied to the current state.
/// - Applying themselves to modify the state.
/// - Providing a descriptive label.
/// - Building a strategy for generating instances of the command.
pub trait Command {
    /// Checks if the command can be applied to the current state.
    /// Returns true if the command can be applied, false otherwise.
    ///
    /// # Arguments
    /// * `state` - The current state to check against.
    fn check(&self, state: &State) -> bool;

    /// Applies the command to the state, modifying it.
    /// This method should only be called if `check` returns true.
    /// It can include assertions to verify correctness.
    ///
    /// # Arguments
    /// * `state` - The state to modify.
    fn apply(&self, state: &mut State);

    /// Returns a human-readable label for the command.
    /// Used for debugging and test output.
    fn label(&self) -> String;

    /// Builds a proptest strategy for generating instances of this command.
    ///
    /// # Arguments
    /// * `ctx` - Test context used to parameterize command generation.
    fn build(ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper>
    where
        Self: Sized;
}

/// Wrapper for command trait objects.
/// This wrapper allows commands to be stored in collections and
/// passed between functions while preserving their concrete type.
/// It provides a convenient way to implement Debug for dynamic Commands.
#[derive(Clone)]
pub struct CommandWrapper {
    /// The wrapped command trait object.
    pub command: Arc<dyn Command>,
}

impl CommandWrapper {
    /// Creates a new command wrapper for the given command.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command to wrap.
    ///
    /// # Examples
    ///
    /// ```
    /// use madhouse::{Command, CommandWrapper, State};
    /// use proptest::prelude::*;
    /// use std::sync::Arc;
    ///
    /// struct TestCommand;
    /// impl Command for TestCommand {
    ///     fn check(&self, _state: &State) -> bool { true }
    ///     fn apply(&self, state: &mut State) { state.last_mined_block += 1; }
    ///     fn label(&self) -> String { "TEST".to_string() }
    ///     fn build(_ctx: Arc<madhouse::TestContext>) ->
    ///         impl Strategy<Value = CommandWrapper> {
    ///         Just(CommandWrapper::new(TestCommand))
    ///     }
    /// }
    ///
    /// let cmd = TestCommand;
    /// let wrapper = CommandWrapper::new(cmd);
    /// assert_eq!(wrapper.command.label(), "TEST");
    /// ```
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
///
/// # Examples
///
/// ```
/// use madhouse::prop_allof;
/// use proptest::prelude::*;
///
/// // Create strategies.
/// let strat1 = Just(1);
/// let strat2 = Just(2);
/// let strat3 = Just(3);
///
/// // Combine them with prop_allof.
/// let combined = prop_allof![strat1, strat2, strat3];
///
/// // The strategy will produce a vec with all values: [1, 2, 3].
/// proptest!(|(v in combined)| {
///     assert_eq!(v, vec![1, 2, 3]);
/// });
/// ```
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

/// Executes a sequence of commands and returns those that were executed.
///
/// This function:
/// 1. Filters commands based on their `check` method.
/// 2. Applies each valid command to the state.
/// 3. Measures execution time for each command.
/// 4. Prints a colored summary of selected and executed commands.
///
/// # Arguments
/// * `commands` - Slice of commands to potentially execute.
/// * `state` - Mutable state that commands will check against and modify.
///
/// # Returns
/// A vector of references to commands that were actually executed.
///
/// # Examples
///
/// ```
/// use madhouse::{Command, CommandWrapper, State, execute_commands};
/// use proptest::prelude::*;
/// use std::sync::Arc;
///
/// // Define a simple command.
/// struct TestCommand;
/// impl Command for TestCommand {
///     fn check(&self, _state: &State) -> bool { true }
///     fn apply(&self, state: &mut State) { state.last_mined_block += 1; }
///     fn label(&self) -> String { "TEST".to_string() }
///     fn build(_ctx: Arc<madhouse::TestContext>) ->
///         impl Strategy<Value = CommandWrapper> {
///         Just(CommandWrapper::new(TestCommand))
///     }
/// }
///
/// // Execute commands.
/// let mut state = State::new();
/// let commands = vec![CommandWrapper::new(TestCommand)];
/// let executed = execute_commands(&commands, &mut state);
///
/// assert_eq!(executed.len(), 1);
/// assert_eq!(state.last_mined_block, 1);
/// ```
pub fn execute_commands<'a>(
    commands: &'a [CommandWrapper],
    state: &mut State,
) -> Vec<&'a CommandWrapper> {
    let mut executed = Vec::with_capacity(commands.len());
    let mut execution_times = Vec::with_capacity(commands.len());

    // ANSI color codes.
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
/// This macro configures proptest to:
/// - Run a single test case (cases = 1).
/// - Skip shrinking (max_shrink_iters = 0).
/// - Use either random or deterministic command generation.
///
/// # Arguments
///
/// * `test_context` - The test context to use for creating commands.
/// * `command1, command2, ...` - The command types to test.
///
/// # Examples
///
/// ```
/// use madhouse::{
///     execute_commands, prop_allof, Command, CommandWrapper, State,
///     TestContext, scenario
/// };
/// use proptest::prelude::Just;
/// use proptest::strategy::Strategy;
/// use std::env;
/// use std::sync::Arc;
///
/// // Define a test command.
/// struct MyCommand;
///
/// impl Command for MyCommand {
///     fn check(&self, _state: &State) -> bool { true }
///     fn apply(&self, state: &mut State) { state.last_mined_block += 1; }
///     fn label(&self) -> String { "TEST".to_string() }
///     fn build(_ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper> {
///         Just(CommandWrapper::new(MyCommand))
///     }
/// }
///
/// // Run the test.
/// let ctx = Arc::new(TestContext::new(vec![]));
/// scenario![ctx, MyCommand];
/// ```
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

        let result = state.add_sortition_block_leader(1, &miner_seed);
        assert!(result.is_ok());
        assert!(state.block_leaders.contains_key(&1));
        assert_eq!(state.block_leaders[&1], miner_seed);

        // Test adding another sortition for the same height
        let result = state.add_sortition_block_leader(1, &miner_seed);
        assert!(result.is_err());
    }

    #[test]
    fn test_test_context() {
        let miner_seeds = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let ctx = TestContext::new(miner_seeds.clone());
        assert_eq!(ctx.miner_seeds, miner_seeds);
    }

    #[test]
    fn test_prop_allof_macro() {
        use proptest::prelude::*;

        // Test with one strategy.
        let strat1 = Just(1);
        let combined1 = prop_allof![strat1];

        proptest!(|(v in combined1)| {
            assert_eq!(v, vec![1]);
        });

        // Test with multiple strategies.
        let strat2 = Just(2);
        let strat3 = Just(3);
        let combined2 = prop_allof![strat1, strat2, strat3];

        proptest!(|(v in combined2)| {
            assert_eq!(v, vec![1, 2, 3]);
        });
    }

    #[test]
    fn test_execute_commands_empty() {
        let commands: Vec<CommandWrapper> = vec![];
        let mut state = State::new();

        let executed = execute_commands(&commands, &mut state);
        assert!(executed.is_empty());
    }

    #[test]
    fn test_execute_commands_all_rejected() {
        struct RejectCommand;

        impl Command for RejectCommand {
            fn check(&self, _state: &State) -> bool {
                false
            }
            fn apply(&self, _state: &mut State) {}
            fn label(&self) -> String {
                "REJECT".to_string()
            }
            fn build(_ctx: Arc<TestContext>) -> impl Strategy<Value = CommandWrapper> {
                Just(CommandWrapper::new(RejectCommand))
            }
        }

        let commands = vec![
            CommandWrapper::new(RejectCommand),
            CommandWrapper::new(RejectCommand),
        ];
        let mut state = State::new();

        let executed = execute_commands(&commands, &mut state);
        assert!(executed.is_empty());
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
