use madhouse::*;
use proptest::prelude::*;
use std::env;
use std::sync::Arc;

// State + Context
#[derive(Debug, Default)]
struct Counter {
    value: u32,
    max: u32,
}
impl State for Counter {}

#[derive(Debug, Clone, Default)]
struct Ctx {}
impl TestContext for Ctx {}

// Commands
struct Inc {
    amount: u32,
}
impl Command<Counter, Ctx> for Inc {
    fn check(&self, s: &Counter) -> bool {
        s.value + self.amount <= s.max
    }
    fn apply(&self, s: &mut Counter) {
        s.value += self.amount;
    }
    fn label(&self) -> String {
        format!("INC({})", self.amount)
    }
    fn build(_: Arc<Ctx>) -> impl Strategy<Value = CommandWrapper<Counter, Ctx>> {
        (1..=5u32).prop_map(|n| CommandWrapper::new(Inc { amount: n }))
    }
}

struct Reset;
impl Command<Counter, Ctx> for Reset {
    fn check(&self, s: &Counter) -> bool {
        s.value > 0
    }
    fn apply(&self, s: &mut Counter) {
        s.value = 0;
    }
    fn label(&self) -> String {
        "RESET".to_string()
    }
    fn build(_: Arc<Ctx>) -> impl Strategy<Value = CommandWrapper<Counter, Ctx>> {
        Just(CommandWrapper::new(Reset))
    }
}

fn main() {
    let ctx = Arc::new(Ctx::default());
    scenario![ctx, Inc, Reset];
}
