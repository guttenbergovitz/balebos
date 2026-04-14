# Balebos Management Makefile

# Variables
APP_NAME = balebos
DOCKER_IMAGE = balebos
DOCKER_IMAGE_DEV = balebos-dev

.PHONY: all build run-ui list scan clean help \
	docker-build docker-dev docker-ui docker-scan docker-list docker-shell docker-clean \
	dev-check dev-test dev-fix dev-doc

all: help

## --- Local Development Commands ---

# Build the project locally
build:
	cargo build --release

# Run TUI locally
run-ui:
	cargo run -- ui

# List domains locally
list:
	cargo run -- list

# Scan a domain locally (usage: make scan domain=example.com)
scan:
	cargo run -- scan $(domain)

# Clean local build artifacts
clean:
	cargo clean

## --- Code Quality & Iteration (DEV) ---

# Quick check if code compiles
dev-check:
	cargo check

# Run all tests
dev-test:
	cargo test

# Automatically fix linting and formatting issues
dev-fix:
	cargo fmt
	cargo clippy --fix --allow-dirty --allow-staged

# Open local documentation in browser
dev-doc:
	cargo doc --open

## --- Docker Commands ---

# Build production Docker image
docker-build:
	docker build -t $(DOCKER_IMAGE) .

# Run dev container with hot reload (mounts source code)
# Usage: make docker-dev [cmd="scan example.com"]
docker-dev:
	docker build -f Dockerfile.dev -t $(DOCKER_IMAGE_DEV) . \
	&& docker run --rm -it \
		-v "$(PWD)/src:/app/src" \
		-v "$(PWD)/Cargo.toml:/app/Cargo.toml" \
		-v "$(PWD)/Cargo.lock:/app/Cargo.lock" \
		-v "$(PWD)/data:/app/data" \
		-v "cargo-registry:/usr/local/cargo/registry" \
		$(DOCKER_IMAGE_DEV) \
		cargo watch -x "run -- $(or $(cmd),ui)"

# Run TUI in production container (interactive)
docker-ui:
	docker run --rm -it -v "$(PWD)/data:/app/data" $(DOCKER_IMAGE) ui

# Scan a domain in production container
# Usage: make docker-scan domain=example.com
docker-scan:
	docker run --rm -v "$(PWD)/data:/app/data" $(DOCKER_IMAGE) scan $(domain)

# List domains in production container
docker-list:
	docker run --rm -v "$(PWD)/data:/app/data" $(DOCKER_IMAGE) list

# Enter shell inside the production container for debugging
docker-shell:
	docker run --rm -it -v "$(PWD)/data:/app/data" --entrypoint bash $(DOCKER_IMAGE)

# Remove Docker images related to balebos
docker-clean:
	docker rmi $(DOCKER_IMAGE) $(DOCKER_IMAGE_DEV) || true

## --- Help ---

help:
	@echo "Balebos Management CLI"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-build          - Build production Docker image"
	@echo "  make docker-dev            - Run dev container with hot reload (TUI)"
	@echo "  make docker-dev cmd='scan example.com' - Dev mode with custom command"
	@echo "  make docker-ui             - Run interactive TUI in production container"
	@echo "  make docker-scan domain=X  - Scan a domain via Docker"
	@echo "  make docker-list           - List assets via Docker"
	@echo "  make docker-shell          - Enter production container shell"
	@echo "  make docker-clean          - Remove Docker images"
	@echo ""
	@echo "Local Commands (requires Rust + scanners in PATH):"
	@echo "  make build                 - Build release binary"
	@echo "  make run-ui                - Run interactive TUI"
	@echo "  make list                  - List assets"
	@echo "  make scan domain=X         - Scan a single domain"
	@echo "  make clean                 - Remove build files"
	@echo ""
	@echo "Dev & Quality:"
	@echo "  make dev-check             - Quick compilation check"
	@echo "  make dev-test              - Run unit and integration tests"
	@echo "  make dev-fix               - Run rustfmt and clippy fix"
	@echo "  make dev-doc               - Generate and open documentation"
