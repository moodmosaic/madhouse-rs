#!/bin/sh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Go up one directory to check if we're in the madhouse repository. We
# assume the script is in the scripts/ subdirectory of the repository.
# This may not be the case if script was copied elsewhere for testing.
# This is to verify the directory structure, before using local paths.
SOURCE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Create a temp directory for our test project.
tmp=$(mktemp -d /tmp/play-XXXXXXXX)
echo "Using temp dir: $tmp"

cleanup() {
    if [ $? -ne 0 ]; then
        echo
        echo "Test failed. Temp project preserved at:"
        echo "  $tmp"
    else
        rm -rf "$tmp"
    fi
}
trap cleanup EXIT

# Create the test project.
cargo new --lib "$tmp/counter" >/dev/null
cd "$tmp/counter"

# Check if this appears to be the madhouse repository.
TOML="$SOURCE_DIR/Cargo.toml"
if [ -f "$TOML" ] && grep -q "name.*madhouse" "$TOML"; then
    echo "✅ Running from within madhouse repository"
    MADHOUSE_DEP="madhouse = { path = \"$SOURCE_DIR\" }"
else
    echo "ℹ️ Running outside madhouse repository - using git dependency"
    MADHOUSE_DEP="madhouse = { git = \
      \"https://github.com/stacks-network/madhouse-rs.git\" }"
fi

# Create Cargo.toml with dependencies.
cat > Cargo.toml <<EOF
[package]
name = "counter"
version = "0.1.0"
edition = "2021"

[dependencies]
$MADHOUSE_DEP
proptest = "1.6.*"
EOF

# Create test file with example.
cat > src/lib.rs <<'EOF'
use madhouse::prelude::*;
use proptest::prelude::*;
use std::sync::Arc;

#[derive(Debug, Default)]
struct Counter {
    value: u32,
}
impl State for Counter {}

#[derive(Clone, Debug, Default)]
struct Ctx {}
impl TestContext for Ctx {}

struct Increment {
    amount: u32,
}
impl Command<Counter, Ctx> for Increment {
    fn check(&self, _: &Counter) -> bool {
        true
    }

    fn apply(&self, state: &mut Counter) {
        state.value += self.amount;
        assert!(
            state.value <= 100,
            "Counter value exceeded maximum allowed: {}",
            state.value
        );
    }

    fn label(&self) -> String {
        format!("INCREMENT({})", self.amount)
    }

    fn build(_: Arc<Ctx>)
        -> impl Strategy<Value = CommandWrapper<Counter, Ctx>>
    {
        (1..=50u32).prop_map(|amount| {
            CommandWrapper::new(Increment { amount })
        })
    }
}

struct Decrement {
    amount: u32,
}
impl Command<Counter, Ctx> for Decrement {
    fn check(&self, state: &Counter) -> bool {
        state.value >= self.amount
    }

    fn apply(&self, state: &mut Counter) {
        state.value -= self.amount;
    }

    fn label(&self) -> String {
        format!("DECREMENT({})", self.amount)
    }

    fn build(_: Arc<Ctx>)
        -> impl Strategy<Value = CommandWrapper<Counter, Ctx>>
    {
        (1..=10u32).prop_map(|amount| {
            CommandWrapper::new(Decrement { amount })
        })
    }
}

#[test]
fn test_counter() {
    let ctx = Arc::new(Ctx::default());
    scenario![ctx, Increment, Decrement, (Increment { amount: 42 })];
}
EOF

"$@" cargo test -- --nocapture
