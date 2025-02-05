use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::process::Command as SysCommand;
use std::sync::Arc;

pub struct ExampleState {
    value: i32,
}

impl ExampleState {
    pub fn new() -> Self {
        Self { value: 0 }
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
}

/// A trait that all commands must implement.
pub trait Command {
    fn check(&self, state: &ExampleState) -> bool;
    fn apply(&self, state: &mut ExampleState);
    fn label(&self) -> &'static str;
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
        "Increment"
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
        "Decrement"
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

        println!(
            "Process Output: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    fn label(&self) -> &'static str {
        "ShellProc"
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
fn test_random_commands() {
    // Set a fixed seed for reproducibility (proptest expects 256-bit seeds).
    let seed = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6,
        0x07, 0x18,
    ];
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed);

    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 1,
            max_shrink_iters: 0, // Disable shrinking.
            failure_persistence: None,
            ..Config::default()
        },
        rng,
    );

    runner
        .run(
            &proptest::collection::vec(
                prop_oneof![
                    Just(CommandWrapper::new(IncrementCommand)),
                    Just(CommandWrapper::new(DecrementCommand)),
                    Just(CommandWrapper::new(ShellProcCommand)),
                ],
                7, // Number of commands per test.
            ),
            |commands| {
                let mut state = ExampleState::new();

                for cmd in &commands {
                    if cmd.command.check(&state) {
                        cmd.command.apply(&mut state);
                    }
                }

                println!("Executed commands: {:?}", commands);
                assert!(state.get_value() >= 0);
                Ok(())
            },
        )
        .unwrap();
}
