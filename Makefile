# Makefile for Aeterna (Memory-Knowledge System)

.PHONY: all build test lint fix clean doc archive-specs

# Default target
all: build lint test

# Build the project
build:
	cargo build

# Run all tests
test:
	cargo test --workspace -- --nocapture

# Run lints
lint:
	cargo clippy --workspace
	cargo fmt --all -- --check

# Fix lints and formatting
fix:
	cargo fmt --all
	cargo clippy --workspace --fix --allow-dirty --allow-staged

# Generate documentation
doc:
	cargo doc --workspace --no-deps --open

# OpenSpec Archive (requires CHANGE_ID variable)
archive-specs:
	@if [ -z "$(CHANGE_ID)" ]; then \
		echo "Error: CHANGE_ID is not set. Usage: make archive-specs CHANGE_ID=add-feature-name"; \
		exit 1; \
	fi
	openspec archive $(CHANGE_ID) --yes

# Clean build artifacts
clean:
	cargo clean
