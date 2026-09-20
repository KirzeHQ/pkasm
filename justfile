default: check

# Formatting
fmt:
  cargo fmt --all

# Verify formatting
fmt-check:
  cargo fmt --all --check

# Build
build:
  cargo build --release

# Tests
test:
  cargo test --all

# linting
lint:
  cargo clippy --all-targets --all-features -- -D warnings

# Check, incl. formatting, linting, and tests
check: fmt-check lint test