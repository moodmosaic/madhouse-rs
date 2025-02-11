use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::process::Command as SysCommand;
use std::sync::Arc;

use proptest::prelude::{Just, Strategy};
use proptest::prop_oneof;
use proptest::proptest;

const MINER_SEEDS: [[u8; 4]; 2] = [[1, 1, 1, 1], [2, 2, 2, 2]];

fn main() {
    println!("Hello, world!");
}

pub struct ExampleState {
    value: i32,
    running_miners: Vec<Vec<u8>>,
    last_mined_block: u64,
    block_commits: HashMap<u64, Vec<Vec<u8>>>,
}

impl ExampleState {
    pub fn new() -> Self {
        Self {
            value: 0,
            running_miners: Vec::new(),
            last_mined_block: 0,
            block_commits: HashMap::new(),
        }
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

    pub fn start_miner(&mut self, miner_seed: Vec<u8>) {
        self.running_miners.push(miner_seed);
        // Log the updated state.
        println!("Running miners: {:?}", self.running_miners);
    }

    pub fn add_block_commit(&mut self, height: u64, miner_seed: Vec<u8>) {
        // Log the updated state.
        println!(
            "Block commit at height {} by miner {:?}",
            height, miner_seed
        );
        let existing_commits = self.block_commits.entry(height).or_insert(Vec::new());
        existing_commits.push(miner_seed);
        println!(
            "Block commiters for height {}: {:?}",
            height,
            self.block_commits.get(&height)
        );
    }
}

/// A trait that all commands must implement.
pub trait Command {
    fn check(&self, state: &ExampleState) -> bool;
    fn apply(&self, state: &mut ExampleState);
    fn label(&self) -> &'static str;
}

pub struct StartMinerCommand {
    miner_seed: Vec<u8>,
}

impl StartMinerCommand {
    pub fn new(miner_seed: Vec<u8>) -> Self {
        // Check validity here. Prevent invalid data from being created.
        Self { miner_seed }
    }
}

impl Command for StartMinerCommand {
    fn check(&self, state: &ExampleState) -> bool {
        // Prevents starting the same miner twice.
        !state
            .running_miners
            .iter()
            .any(|running| running == &self.miner_seed)
    }

    fn apply(&self, state: &mut ExampleState) {
        println!("Starting miner with seed: {:?}", self.miner_seed);
        state.start_miner(self.miner_seed.clone());
    }

    fn label(&self) -> &'static str {
        "START_MINER"
    }
}

pub struct SubmitBlockCommitCommand {
    miner_seed: Vec<u8>,
}

impl SubmitBlockCommitCommand {
    pub fn new(miner_seed: Vec<u8>) -> Self {
        // Check validity here. Prevent invalid data from being created.
        Self { miner_seed }
    }
}

impl Command for SubmitBlockCommitCommand {
    fn check(&self, state: &ExampleState) -> bool {
        // A miner can submit a block commit only if:
        // 1. The miner is running.
        // 2. The miner has not submitted a block commit at the same height.
        state
            .running_miners
            .iter()
            .any(|running| running == &self.miner_seed)
            && !state
                .block_commits
                .get(&(state.last_mined_block + 1))
                .map(|commits| commits.contains(&self.miner_seed))
                .unwrap_or(false)
    }

    fn apply(&self, state: &mut ExampleState) {
        println!(
            "Submitting block commit at height {} by miner {:?}",
            state.last_mined_block + 1,
            self.miner_seed
        );
        state.add_block_commit(state.last_mined_block + 1, self.miner_seed.clone());
    }

    fn label(&self) -> &'static str {
        "SUBMIT_BLOCK_COMMIT"
    }
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

    fn label(&self) -> &'static str {
        "A"
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

    fn label(&self) -> &'static str {
        "B"
    }
}

/// Command that spawns an external process.
#[derive(Debug, Clone)]
pub struct ShellProcCommand;

impl Command for ShellProcCommand {
    fn check(&self, _state: &ExampleState) -> bool {
        true // Always allowed.
    }

    fn apply(&self, _state: &mut ExampleState) {
        let output = SysCommand::new("echo")
            .arg("Hello, world!")
            .output()
            .expect("Failed to execute process");

        println!("{}", String::from_utf8_lossy(&output.stdout).trim_end());
    }

    fn label(&self) -> &'static str {
        "C"
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
              Just(CommandWrapper::new(IncrementCommand)),
              Just(CommandWrapper::new(DecrementCommand)),
              Just(CommandWrapper::new(ShellProcCommand)),
              proptest::sample::select(&MINER_SEEDS)
              .prop_map(|seed| CommandWrapper::new(StartMinerCommand::new(seed.to_vec()))),
              proptest::sample::select(&MINER_SEEDS)
              .prop_map(|seed| CommandWrapper::new(SubmitBlockCommitCommand::new(seed.to_vec()))),
          ],
          1..10, // Change to something higher like 70.
      )
  ) {
      println!("\n=== New Test Run ===\n");
      let mut state = ExampleState::new();
      for cmd in &commands {
          if cmd.command.check(&state) {
              cmd.command.apply(&mut state);
          }
      }
      println!("Executed commands: {:?}", commands);
        // Fail only if the state gets too high.
        // This condition may be hit only rarely.
        assert!(state.get_value() < 10,
                "State too high: {}", state.get_value());
  }
}
