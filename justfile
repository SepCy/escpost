# Build, test, and run escpost either in Docker (reproducible, no host toolchain)
# or natively (host Rust toolchain; runs as a real host binary). See README.md.
# Every recipe here has an identical `make` target.

# Containerized cargo, via the compose `test` service.
docker_cargo := "docker compose run --rm test cargo"

# List available recipes.
default:
    @just --list

# --- Docker (no host toolchain) ---

# Compile the CLI in the container.
docker-build:
    {{docker_cargo}} build -p escpost-cli

# Run the test suite in the container.
docker-test:
    {{docker_cargo}} test --workspace --exclude escpost-python

# Run the CLI in the container, e.g. `just docker-run serve --no-open`.
docker-run *args:
    {{docker_cargo}} run -q -p escpost-cli -- {{args}}

# --- Native (host Rust toolchain) ---

# Build target/release/escpost.
native-build:
    cargo build --release -p escpost-cli

# Run the test suite on the host.
native-test:
    cargo test --workspace --exclude escpost-python

# Run the CLI on the host, e.g. `just native-run serve`.
native-run *args:
    cargo run -q -p escpost-cli -- {{args}}

# --- Utilities ---

# Regenerate profiles/.generated/profiles.json.
pack:
    {{docker_cargo}} run -q -p escpost-profiles --bin compile-profile-pack -- profiles/.escpos-printer-db/dist/capabilities.json profiles profiles/.generated/profiles.json

# Build and test the Python render binding.
python-test:
    scripts/python-binding-test
