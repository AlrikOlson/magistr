# magistr — development recipes

# All required project checks
gate: check test lint structure
    ./scripts/lint-duplication.sh
    ./scripts/machete-gate.sh
    ./scripts/screenshot-gate.sh
    ./scripts/release-latency-gate.sh
    ./scripts/nextest-config-gate.sh
    ./scripts/legacy-gui-ratchet.sh

# Compile check all targets
check:
    cargo check --workspace --all-targets

# Run tests
test:
    cargo nextest run --workspace

# Run clippy with warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format code
fmt:
    cargo fmt

# Run structure lint
structure:
    ./scripts/lint-structure.sh

# Run magistr CLI
run *args:
    cargo run -- {{args}}

# rm + cp, never cp in place: overwriting a running binary SIGKILLs live magistr processes
# Build the release binary and install it globally on PATH
install:
    cargo build --release -p magistr-cli
    rm -f "${CARGO_HOME:-$HOME/.cargo}/bin/magistr"
    cp target/release/magistr "${CARGO_HOME:-$HOME/.cargo}/bin/magistr"
    @echo "installed $(magistr --version) at ${CARGO_HOME:-$HOME/.cargo}/bin/magistr"

# Website

# Start website dev server
www-dev:
    cd apps/website && npm run dev

# Build website
www-build:
    cd apps/website && npm run build

# Preview production build
www-preview:
    cd apps/website && npm run build && npm run preview

# Run website tests
www-test:
    cd apps/website && npm run test
