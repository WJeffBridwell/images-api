#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo "🧪 Running test suite for images-api"

# Run unit tests
echo -e "\n${GREEN}Running unit tests...${NC}"
RUST_LOG=debug cargo test --lib -- --nocapture

# Run integration tests
echo -e "\n${GREEN}Running integration tests...${NC}"
RUST_LOG=debug cargo test --test "integration_*" -- --nocapture

# Run benchmarks
echo -e "\n${GREEN}Running performance benchmarks...${NC}"
cargo bench

# Check code formatting
echo -e "\n${GREEN}Checking code formatting...${NC}"
cargo fmt -- --check

# Run clippy for linting
echo -e "\n${GREEN}Running linter...${NC}"
cargo clippy -- -D warnings

echo -e "\n${GREEN}All tests completed successfully!${NC}"
