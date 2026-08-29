# Native development workflows and repository maintenance utilities. Container
# development uses Docker Compose directly; see README.md.

# Containerized cargo, via the compose `test` service.
docker_cargo := "docker compose run --rm test cargo"

# List available recipes.
default:
    @just --list

# --- Host toolchains ---

# Install the frontend dependencies. The Vite server needs them.
frontend-install:
    cd crates/escpost/frontend && bun install --frozen-lockfile

# Build the web app bundle into crates/escpost/frontend/dist.
frontend-build:
    cd crates/escpost/frontend && bun install --frozen-lockfile && bun run build

# Build target/release/escpost.
build: frontend-build
    cargo build --release -p escpost

# Run the test suite on the host.
test: frontend-build
    cargo test --workspace --exclude escpost-python

# A debug build reads the web app from disk at run time. Use `frontend-build`
# first if the CLI must serve it.
[doc("Run the CLI on the host, e.g. `just run serve`.")]
run *args:
    cargo run -q -p escpost -- {{args}}

# Run the backend and Vite development server with host toolchains.
web-dev: frontend-install
    scripts/native-web-dev

# --- Utilities ---

# Clear the shared Docker Cargo build cache.
docker-cargo-clean:
    docker compose run --rm --no-deps --entrypoint sh escpost -c 'find "$CARGO_TARGET_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +'

# Set the lockstep workspace version and refresh Cargo.lock.
[doc("Set every publishable Rust crate to one release version.")]
set-version version:
    python3 scripts/set-workspace-version {{quote(version)}}
    cargo metadata --format-version 1 --no-deps > /dev/null

# Regenerate crates/escpost-profiles/profiles/.generated/profiles.json.
generate-profile-pack:
    {{docker_cargo}} run -q -p escpost-profiles --bin compile-profile-pack -- crates/escpost-profiles/profiles/.escpos-printer-db/dist/capabilities.json crates/escpost-profiles/profiles crates/escpost-profiles/profiles/.generated/profiles.json

# Build and test the Python render binding.
python-test:
    scripts/python-binding-test

# Publish escpost-render and escpost-profiles first, because escpost needs
# them at the versions in this workspace.
[doc("Publish the CLI to crates.io with the web app built in.")]
publish: frontend-build
    cargo publish -p escpost
