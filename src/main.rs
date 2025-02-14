#![allow(unused)]
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use proptest::prelude::{Just, Strategy};
use proptest::prop_oneof;
use proptest::proptest;

const MINER_SEEDS: [[u8; 4]; 2] = [[1, 1, 1, 1], [2, 2, 2, 2]];

fn main() {
    println!("Hello, world!");
}

#[derive(Default)]
pub struct State {
    running_miners: HashSet<Vec<u8>>,
    last_mined_block: u64,
    block_commits: HashMap<u64, HashSet<Vec<u8>>>,
    block_leaders: HashMap<u64, Vec<u8>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            last_mined_block: 0,
            ..Default::default()
        }
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

    fn label(&self) -> &'static str {
        "WAIT_FOR_BLOCKS"
    }

    fn build() -> impl Strategy<Value = CommandWrapper> {
        (1u64..5).prop_map(|val| CommandWrapper::new(WaitForBlocksCommand::new(val)))
    }
}

/// A trait that all commands must implement.
pub trait Command {
    fn check(&self, state: &State) -> bool;
    fn apply(&self, state: &mut State);
    fn label(&self) -> &'static str;
    fn build() -> impl Strategy<Value = CommandWrapper>
    where
        Self: Sized;
}

struct StartMinerCommand {
    miner_seed: Vec<u8>,
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
        !state.running_miners.contains(&self.miner_seed)
    }

    fn apply(&self, state: &mut State) {
        println!("Starting miner with seed: {:?}", self.miner_seed);
        state.start_miner(&self.miner_seed);
    }

    fn label(&self) -> &'static str {
        "START_MINER"
    }

    fn build() -> impl Strategy<Value = CommandWrapper> {
        proptest::sample::select(&MINER_SEEDS)
            .prop_map(|seed| CommandWrapper::new(StartMinerCommand::new(&seed)))
    }
}

struct SubmitBlockCommitCommand {
    miner_seed: Vec<u8>,
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
        state.running_miners.contains(&self.miner_seed)
            && !state
                .block_commits
                .get(&(state.last_mined_block + 1))
                .map(|commits| commits.contains(&self.miner_seed))
                .unwrap_or(false)
    }

    fn apply(&self, state: &mut State) {
        println!(
            "Submitting block commit at height {} by miner {:?}",
            state.last_mined_block + 1,
            self.miner_seed
        );
        state.add_block_commit(state.last_mined_block + 1, &self.miner_seed);
    }

    fn label(&self) -> &'static str {
        "SUBMIT_BLOCK_COMMIT"
    }

    fn build() -> impl Strategy<Value = CommandWrapper> {
        proptest::sample::select(&MINER_SEEDS)
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
            .get(&(state.last_mined_block + 1))
            .map(|commits| !commits.is_empty())
            .unwrap_or(false)
            && !state
                .block_leaders
                .contains_key(&(state.last_mined_block + 1))
    }

    fn apply(&self, state: &mut State) {
        // Simulate a random leader by picking an index from the list of miners
        // that submitted a block commit. For deterministic leader selection,
        // we are hashing the height and the set of committers and then picking
        // the leader based on the hash value.
        let next_block_height = state.last_mined_block + 1;

        let block_commits_next_block = state
            .block_commits
            .get(&next_block_height)
            .expect("No commits found, but check() should have prevented this.")
            .clone();

        let mut sorted_committers: Vec<Vec<u8>> =
            block_commits_next_block.iter().cloned().collect();

        sorted_committers.sort();

        let mut hasher = DefaultHasher::default();
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

    fn label(&self) -> &'static str {
        "SORTITION"
    }

    fn build() -> impl Strategy<Value = CommandWrapper> {
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

proptest! {
  #[test]
  fn stateful_test(
      commands in proptest::collection::vec(
          prop_oneof![
              SortitionCommand::build(),
              StartMinerCommand::build(),
              SubmitBlockCommitCommand::build(),
              WaitForBlocksCommand::build(),
          ],
          1..16, // Change to something higher like 70.
      )
  ) {
      println!("\n=== New Test Run ===\n");
      let mut state = State::default();
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
  }
}
