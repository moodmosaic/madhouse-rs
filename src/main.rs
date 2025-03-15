use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

#[cfg(test)]
use proptest::collection::vec;
use proptest::prelude::{Just, Strategy};
#[cfg(test)]
use proptest::prop_oneof;
#[cfg(test)]
use proptest::proptest;

fn main() {
    println!("Hello, world!");
}

type MinerSeed = Vec<u8>;

#[derive(Clone, Debug)]
pub struct TestContext {
    miner_seeds: Vec<MinerSeed>,
}

#[cfg(test)]
impl TestContext {
    fn new(miner_seeds: Vec<MinerSeed>) -> Self {
        Self { miner_seeds }
    }
}

#[derive(Default)]
pub struct State {
    running_miners: HashSet<MinerSeed>,
    last_mined_block: u64,
    block_commits: HashMap<u64, HashSet<MinerSeed>>,
    block_leaders: HashMap<u64, MinerSeed>,
}

impl State {
    pub fn new() -> Self {
        Self {
            running_miners: HashSet::new(),
            last_mined_block: 0,
            block_commits: HashMap::new(),
            block_leaders: HashMap::new(),
        }
    }

    pub fn is_miner_running(&self, seed: &MinerSeed) -> bool {
        self.running_miners.contains(seed)
    }

    pub fn next_block_height(&self) -> u64 {
        self.last_mined_block + 1
    }

    pub fn start_miner(&mut self, miner_seed: &[u8]) {
        self.running_miners.insert(miner_seed.to_vec());
        println!("Running miners: {:?}", self.running_miners);
    }

    pub fn add_block_commit(&mut self, height: u64, miner_seed: &[u8]) {
        println!(
            "Block commit at height {} by miner {:?}",
            height, miner_seed
        );
        let existing_commits = self.block_commits.entry(height).or_default();
        existing_commits.insert(miner_seed.to_vec());
        println!(
            "Block commiters for height {}: {:?}",
            height,
            self.block_commits.get(&height)
        );
    }

    pub fn add_sortition_block_leader(&mut self, height: u64, miner_seed: &[u8]) {
        match self.block_leaders.get(&height) {
            Some(_) => {
                panic!(
                    "FATAL: For height {} the sortition already happened!",
                    height
                )
            }
            None => {
                self.block_leaders.insert(height, miner_seed.to_vec());
                println!(
                    "Block leader at height {} is miner {:?}",
                    height, miner_seed
                );
            }
        }
    }
}

struct WaitForBlocksCommand {
    count: u64,
}

impl WaitForBlocksCommand {
    pub fn new(count: u64) -> Self {
        Self { count }
    }
}

impl Command for WaitForBlocksCommand {
    fn check(&self, _state: &State) -> bool {
        true
    }

    fn apply(&self, state: &mut State) {
        println!("{} blocks mined.", self.count);
        state.last_mined_block += self.count;
    }

    fn label(&self) -> String {
        "WAIT_FOR_BLOCKS".to_string()
    }

    fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
        (1u64..5).prop_map(|val| CommandWrapper::new(WaitForBlocksCommand::new(val)))
    }
}

/// A trait that all commands must implement.
pub trait Command {
    fn check(&self, state: &State) -> bool;
    fn apply(&self, state: &mut State);
    fn label(&self) -> String;
    fn build(ctx: &TestContext) -> impl Strategy<Value = CommandWrapper>
    where
        Self: Sized;
}

struct StartMinerCommand {
    miner_seed: MinerSeed,
}

impl StartMinerCommand {
    pub fn new(miner_seed: &[u8]) -> Self {
        // Check validity here. Prevent invalid data from being created.
        Self {
            miner_seed: miner_seed.to_vec(),
        }
    }
}

impl Command for StartMinerCommand {
    fn check(&self, state: &State) -> bool {
        // Prevents starting the same miner twice.
        !state.is_miner_running(&self.miner_seed)
    }

    fn apply(&self, state: &mut State) {
        // Act

        // This is a dummy action. In a real-world scenario, this could be
        // replaced with a controller call to start the miner.
        let actual = std::process::Command::new("date")
            .arg("+%Y")
            .output()
            .expect("Failed to execute process");

        // Assert

        // This is a dummy assertion. In a real-world scenario, this could be
        // replaced with a more meaningful assertion, like checking a log to
        // ensure that the miner has started successfully.
        let actual_str = String::from_utf8_lossy(&actual.stdout)
            .trim_end()
            .to_string();
        let current_year = "2025";
        assert_eq!(actual_str, current_year, "Year mismatch!");

        // Update the locally tracked state.
        state.start_miner(&self.miner_seed);
    }

    fn label(&self) -> String {
        format!("START_MINER({:?})", self.miner_seed)
    }

    fn build(ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
        proptest::sample::select(ctx.miner_seeds.clone())
            .prop_map(|seed| CommandWrapper::new(StartMinerCommand::new(&seed)))
    }
}

struct SubmitBlockCommitCommand {
    miner_seed: MinerSeed,
}

impl SubmitBlockCommitCommand {
    pub fn new(miner_seed: &[u8]) -> Self {
        // Check validity here. Prevent invalid data from being created.
        Self {
            miner_seed: miner_seed.to_vec(),
        }
    }
}

impl Command for SubmitBlockCommitCommand {
    fn check(&self, state: &State) -> bool {
        // A miner can submit a block commit only if:
        // 1. The miner is running.
        // 2. The miner has not submitted a block commit at the same height.
        state.is_miner_running(&self.miner_seed)
            && !state
                .block_commits
                .get(&(state.next_block_height()))
                .map(|commits| commits.contains(&self.miner_seed))
                .unwrap_or(false)
    }

    fn apply(&self, state: &mut State) {
        println!(
            "Submitting block commit at height {} by miner {:?}",
            state.next_block_height(),
            self.miner_seed
        );

        let block_commits_count_before = state
            .block_commits
            .get(&(state.next_block_height()))
            .map(|commits| commits.len())
            .unwrap_or(0);

        state.add_block_commit(state.next_block_height(), &self.miner_seed);

        // This is the place where a general truth about the block commit
        // should be checked. The following is just an example assertion. This
        // can also check the state of the `SUT` (System Under Test) after the
        // command is applied.
        let block_commits_count_after = state
            .block_commits
            .get(&(state.next_block_height()))
            .map(|commits| commits.len())
            .unwrap_or(0);

        assert_eq!(block_commits_count_after, block_commits_count_before + 1);
    }

    fn label(&self) -> String {
        "SUBMIT_BLOCK_COMMIT".to_string()
    }

    fn build(ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
        proptest::sample::select(ctx.miner_seeds.clone())
            .prop_map(|seed| CommandWrapper::new(SubmitBlockCommitCommand::new(&seed)))
    }
}

struct SortitionCommand;

impl Command for SortitionCommand {
    fn check(&self, state: &State) -> bool {
        // The sortition can happen only if:
        // 1. At least one miner submitted a block commit for the upcoming
        // block.
        // 2. The sortition has not happened yet for the upcoming block.
        state
            .block_commits
            .get(&(state.next_block_height()))
            .map(|commits| !commits.is_empty())
            .unwrap_or(false)
            && !state
                .block_leaders
                .contains_key(&(state.next_block_height()))
    }

    fn apply(&self, state: &mut State) {
        // Simulate a random leader by picking an index from the list of miners
        // that submitted a block commit. For deterministic leader selection,
        // we are hashing the height and the set of committers and then picking
        // the leader based on the hash value.
        let next_block_height = state.next_block_height();

        let block_commits_next_block = state
            .block_commits
            .get(&next_block_height)
            .expect("No commits found, but check() should have prevented this.")
            .clone();

        let mut sorted_committers: Vec<MinerSeed> =
            block_commits_next_block.iter().cloned().collect();

        sorted_committers.sort();

        let mut hasher = DefaultHasher::new();
        next_block_height.hash(&mut hasher);

        for commit in &sorted_committers {
            commit.hash(&mut hasher);
        }

        let hash_value = hasher.finish();

        // Pick the leader deterministically.
        let leader_index = (hash_value as usize) % sorted_committers.len();
        let leader = sorted_committers.get(leader_index).unwrap();

        println!(
            "Sortition leader at height {} is miner {:?}",
            next_block_height, leader
        );

        state.add_sortition_block_leader(next_block_height, leader);
    }

    fn label(&self) -> String {
        "SORTITION".to_string()
    }

    fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
        Just(CommandWrapper::new(SortitionCommand))
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
        write!(f, "{}", self.command.label()) // Print command label.
    }
}

#[test]
fn stateful_test() {
    let test_context = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);

    let config = proptest::test_runner::Config { cases: 1, ..Default::default() };

    proptest!(config, |(commands in vec(
        prop_oneof![
            SortitionCommand::build(&test_context),
            StartMinerCommand::build(&test_context),
            SubmitBlockCommitCommand::build(&test_context),
            WaitForBlocksCommand::build(&test_context),
        ],
        1..16,
    ))| {
        println!("\n=== New Test Run (manual) ===\n");

        let mut state = State::new();

        let mut executed_commands = Vec::with_capacity(commands.len());

        for cmd in &commands {
            if cmd.command.check(&state) {
                cmd.command.apply(&mut state);
                executed_commands.push(cmd);
            }
        }

        println!("\nSelected commands:\n");
        for command in &commands {
            println!("{:?}", command);
        }

        println!("\nExecuted commands:\n");
        for command in &executed_commands {
            println!("{:?}", command);
        }
    });
}

#[test]
fn hardcoded_sequence_test() {
    let test_ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);
    let seed_1 = &test_ctx.miner_seeds[0];
    let seed_2 = &test_ctx.miner_seeds[1];

    let mut state = State::new();

    // Start 2 miners.
    let start_miner_1 = StartMinerCommand::new(seed_1);
    assert!(start_miner_1.check(&state));
    start_miner_1.apply(&mut state);

    let start_miner_2 = StartMinerCommand::new(seed_2);
    assert!(start_miner_2.check(&state));
    start_miner_2.apply(&mut state);

    // Submit block commit by miner 1.
    let submit_block_commit_1 = SubmitBlockCommitCommand::new(seed_1);
    assert!(submit_block_commit_1.check(&state));
    submit_block_commit_1.apply(&mut state);

    // Submit block commit by miner 2.
    let submit_block_commit_2 = SubmitBlockCommitCommand::new(seed_2);
    assert!(submit_block_commit_2.check(&state));
    submit_block_commit_2.apply(&mut state);

    // Sortition.
    let sortition = SortitionCommand;
    assert!(sortition.check(&state));
    sortition.apply(&mut state);
    assert!(state.block_leaders.contains_key(&1));
    let leader = state.block_leaders.get(&1).unwrap();
    assert!(leader == seed_1 || leader == seed_2);

    // Wait for 2 blocks.
    let wait_for_blocks = WaitForBlocksCommand::new(2);
    assert!(wait_for_blocks.check(&state));
    wait_for_blocks.apply(&mut state);
    assert_eq!(state.last_mined_block, 2);
}

#[cfg(test)]
macro_rules! madhouse {
    ($test_context:expr, [ $( $command:ident ),* ], $min:expr, $max:expr) => {
        let config = proptest::test_runner::Config { cases: 1, ..Default::default() };

        proptest!(config, |(commands in vec(
            prop_oneof![
                $( $command::build(&$test_context), )*
            ],
            $min..$max,
        ))| {
            println!("\n=== New Test Run (macro) ===\n");

            let mut state = State::new();
            let mut executed_commands = Vec::with_capacity(commands.len());

            for cmd in &commands {
                if cmd.command.check(&state) {
                    cmd.command.apply(&mut state);
                    executed_commands.push(cmd);
                }
            }

            println!("\nSelected commands:\n");
            for command in &commands {
                println!("{:?}", command);
            }

            println!("\nExecuted commands:\n");
            for command in &executed_commands {
                println!("{:?}", command);
            }
        });
    };
}

#[test]
fn macro_stateful_test() {
    let test_context = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);

    madhouse!(
        test_context,
        [
            StartMinerCommand,
            SubmitBlockCommitCommand,
            SortitionCommand,
            WaitForBlocksCommand
        ],
        1,  // Min
        16  // Max
    );
}
