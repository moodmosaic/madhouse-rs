use madhouse::{scenario, Command, CommandWrapper, State, TestContext};
use proptest::prelude::{Just, Strategy};
use proptest::strategy::ValueTree;
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};

fn main() {
    println!("Running integration example...");

    // Check if we're in random or deterministic mode.
    let mode = if env::var("MADHOUSE").map(|v| v == "1").unwrap_or(false) {
        "RANDOM"
    } else {
        "DETERMINISTIC"
    };

    println!("Execution mode: {}", mode);

    let test_context = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);

    scenario!(
        test_context,
        [
            StartMinerCommand,
            SubmitBlockCommitCommand,
            SortitionCommand,
            WaitForBlocksCommand
        ]
    );
}

struct WaitForBlocksCommand {
    count: u64,
}

impl WaitForBlocksCommand {
    fn new(count: u64) -> Self {
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

struct StartMinerCommand {
    miner_seed: Vec<u8>,
}

impl StartMinerCommand {
    fn new(miner_seed: &[u8]) -> Self {
        Self {
            miner_seed: miner_seed.to_vec(),
        }
    }
}

impl Command for StartMinerCommand {
    fn check(&self, state: &State) -> bool {
        !state.is_miner_running(&self.miner_seed) // Prevent duplicate starts.
    }

    fn apply(&self, state: &mut State) {
        println!("Starting miner with seed {:?}", self.miner_seed);
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
    miner_seed: Vec<u8>,
}

impl SubmitBlockCommitCommand {
    fn new(miner_seed: &[u8]) -> Self {
        Self {
            miner_seed: miner_seed.to_vec(),
        }
    }
}

impl Command for SubmitBlockCommitCommand {
    fn check(&self, state: &State) -> bool {
        let height = state.next_block_height();
        state.is_miner_running(&self.miner_seed)
            && !state
                .block_commits
                .get(&height)
                .map(|commits| commits.contains(&self.miner_seed))
                .unwrap_or(false)
    }

    fn apply(&self, state: &mut State) {
        let height = state.next_block_height();
        println!(
            "Submitting block commit at height {} by {:?}",
            height, self.miner_seed
        );

        let before = state
            .block_commits
            .get(&height)
            .map(|c| c.len())
            .unwrap_or(0);

        state.add_block_commit(height, &self.miner_seed);

        let after = state
            .block_commits
            .get(&height)
            .map(|c| c.len())
            .unwrap_or(0);

        assert_eq!(after, before + 1);
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
        let height = state.next_block_height();
        state
            .block_commits
            .get(&height)
            .map(|c| !c.is_empty())
            .unwrap_or(false)
            && !state.block_leaders.contains_key(&height)
    }

    fn apply(&self, state: &mut State) {
        let height = state.next_block_height();
        let commits = state
            .block_commits
            .get(&height)
            .expect("Check should prevent this.")
            .clone();

        let mut sorted_committers: Vec<Vec<u8>> = commits.iter().cloned().collect();
        sorted_committers.sort();

        let mut hasher = DefaultHasher::new();
        height.hash(&mut hasher);
        for commit in &sorted_committers {
            commit.hash(&mut hasher);
        }

        let leader = &sorted_committers[(hasher.finish() as usize) % sorted_committers.len()];

        println!("Sortition leader at height {} is {:?}", height, leader);
        state.add_sortition_block_leader(height, leader);
    }

    fn label(&self) -> String {
        "SORTITION".to_string()
    }

    fn build(_ctx: &TestContext) -> impl Strategy<Value = CommandWrapper> {
        Just(CommandWrapper::new(SortitionCommand))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_deterministic_execution() {
        // Ensure we're in deterministic mode.
        env::remove_var("MADHOUSE");

        let ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);

        // This should execute deterministically
        let mut state = State::new();
        let mut test_runner = proptest::test_runner::TestRunner::default();
        let mut commands = Vec::new();

        // Manually create the same sequence the macro would in deterministic mode
        let strategy1 = StartMinerCommand::build(&ctx);
        if let Ok(value) = strategy1.new_tree(&mut test_runner).map(|v| v.current()) {
            commands.push(value);
        }

        let strategy2 = SubmitBlockCommitCommand::build(&ctx);
        if let Ok(value) = strategy2.new_tree(&mut test_runner).map(|v| v.current()) {
            commands.push(value);
        }

        let strategy3 = SortitionCommand::build(&ctx);
        if let Ok(value) = strategy3.new_tree(&mut test_runner).map(|v| v.current()) {
            commands.push(value);
        }

        let strategy4 = WaitForBlocksCommand::build(&ctx);
        if let Ok(value) = strategy4.new_tree(&mut test_runner).map(|v| v.current()) {
            commands.push(value);
        }

        // Execute the commands manually.
        let mut executed = Vec::new();
        for cmd in &commands {
            if cmd.command.check(&state) {
                cmd.command.apply(&mut state);
                executed.push(cmd);
            }
        }

        // Verify that at least some commands executed (exact count depends on preconditions)
        assert!(!executed.is_empty());
    }

    #[test]
    fn test_random_execution() {
        // Force random mode.
        env::set_var("MADHOUSE", "1");

        let ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);

        scenario!(
            ctx,
            [
                StartMinerCommand,
                SubmitBlockCommitCommand,
                SortitionCommand,
                WaitForBlocksCommand
            ]
        );

        // Reset environment for other tests
        env::remove_var("MADHOUSE");
    }

    #[test]
    fn stateful_test() {
        let ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);
        let config = proptest::test_runner::Config {
            cases: 1,
            failure_persistence: Some(Box::new(
                proptest::test_runner::FileFailurePersistence::WithSource("integration"),
            )),
            ..Default::default()
        };

        proptest::proptest!(config, |(commands in proptest::collection::vec(
            proptest::prop_oneof![
                SortitionCommand::build(&ctx),
                StartMinerCommand::build(&ctx),
                SubmitBlockCommitCommand::build(&ctx),
                WaitForBlocksCommand::build(&ctx),
            ],
            1..16,
        ))| {
            println!("\n=== New Test Run ===\n");
            let mut state = State::new();
            let mut executed = Vec::with_capacity(commands.len());

            for cmd in &commands {
                if cmd.command.check(&state) {
                    cmd.command.apply(&mut state);
                    executed.push(cmd);
                }
            }

            println!("Selected:");
            for (i, cmd) in commands.iter().enumerate() {
                println!("{:02}. {}", i + 1, cmd.command.label());
            }

            println!("Executed:");
            for (i, cmd) in executed.iter().enumerate() {
                println!("{:02}. {}", i + 1, cmd.command.label());
            }
        });
    }

    #[test]
    fn hardcoded_sequence_test() {
        let ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);
        let (seed1, seed2) = (&ctx.miner_seeds[0], &ctx.miner_seeds[1]);

        let mut state = State::new();

        let start1 = StartMinerCommand::new(seed1);
        assert!(start1.check(&state));
        start1.apply(&mut state);

        let start2 = StartMinerCommand::new(seed2);
        assert!(start2.check(&state));
        start2.apply(&mut state);

        let commit1 = SubmitBlockCommitCommand::new(seed1);
        assert!(commit1.check(&state));
        commit1.apply(&mut state);

        let commit2 = SubmitBlockCommitCommand::new(seed2);
        assert!(commit2.check(&state));
        commit2.apply(&mut state);

        let sortition = SortitionCommand;
        assert!(sortition.check(&state));
        sortition.apply(&mut state);
        assert!(state.block_leaders.contains_key(&1));

        let leader = state.block_leaders.get(&1).unwrap();
        assert!(leader == seed1 || leader == seed2);

        let wait = WaitForBlocksCommand::new(2);
        assert!(wait.check(&state));
        wait.apply(&mut state);
        assert_eq!(state.last_mined_block, 2);
    }

    #[test]
    fn macro_stateful_test() {
        let ctx = TestContext::new(vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2]]);
        scenario!(
            ctx,
            [
                StartMinerCommand,
                SubmitBlockCommitCommand,
                SortitionCommand,
                WaitForBlocksCommand
            ]
        );
    }
}
