default:
    @just --list

# Full pre-PR chain: format check, lint, tests
ci: fmt-check lint test

build:
    cargo build

build-release:
    cargo build --release

desktop-build:
    cd desktop && npm run build

desktop-build-no-bundle:
    cd desktop && npm run build:no-bundle

desktop-build-linux:
    cd desktop && npm run build:linux

desktop-build-windows:
    cd desktop && npm run build:windows

desktop-build-macos:
    cd desktop && npm run build:macos

test:
    cargo test --all-targets

# Filesystem seam integration tests (what CI runs on macOS/Windows)
test-fs:
    cargo test --test config_fs

# Clippy with warnings as errors
lint:
    cargo clippy --all-targets --all-features -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

release:
    bash scripts/release.sh

# Regenerate DETECTION_COVERAGE.md from the live catalog
patterns-doc:
    cargo run --quiet --features patterns-doc --bin patterns_doc > DETECTION_COVERAGE.md

clean:
    cargo clean
