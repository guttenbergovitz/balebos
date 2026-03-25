# Balebos Management Makefile

# Variables
APP_NAME = balebos
DOCKER_IMAGE = balebos
STORAGE_TYPE = duckdb
LOCAL_DB_PATH = data/inventory.duckdb
# Path to Homebrew lib for macOS local development
# This may need adjustment depending on user system
BREW_LIB_PATH = /opt/homebrew/lib

.PHONY: all build run-ui list scan clean help docker-build docker-ui docker-scan docker-clean dev-fix dev-test dev-check dev-shell

all: help

## --- Local Development Commands ---

# Build the project locally
build:
	LIBRARY_PATH="$(BREW_LIB_PATH)" cargo build --release

# Run TUI locally
run-ui:
	LIBRARY_PATH="$(BREW_LIB_PATH)" BALEBOS_STORAGE=$(STORAGE_TYPE) cargo run -- ui

# List domains locally
list:
	LIBRARY_PATH="$(BREW_LIB_PATH)" BALEBOS_STORAGE=$(STORAGE_TYPE) cargo run -- list

# Scan a domain locally (usage: make scan domain=example.com)
scan:
	LIBRARY_PATH="$(BREW_LIB_PATH)" BALEBOS_STORAGE=$(STORAGE_TYPE) cargo run -- scan $(domain)

# Clean local build artifacts and database
clean:
	cargo clean
	rm -f $(LOCAL_DB_PATH)

## --- Code Quality & Iteration (DEV) ---

# Quick check if code compiles
dev-check:
	LIBRARY_PATH="$(BREW_LIB_PATH)" cargo check

# Run all tests
dev-test:
	LIBRARY_PATH="$(BREW_LIB_PATH)" cargo test

# Automatically fix linting and formatting issues
dev-fix:
	cargo fmt
	LIBRARY_PATH="$(BREW_LIB_PATH)" cargo clippy --fix --allow-dirty --allow-staged

# Open local documentation in browser
dev-doc:
	LIBRARY_PATH="$(BREW_LIB_PATH)" cargo doc --open

## --- Docker Commands ---

# Build the Docker container
docker-build:
	docker compose build

# Run TUI in Docker (interactive)
docker-ui:
	docker compose run --rm balebos ui

# Scan a domain in Docker (usage: make docker-scan domain=example.com)
docker-scan:
	docker compose run --rm balebos scan $(domain)

# List domains in Docker
docker-list:
	docker compose run --rm balebos list

# Enter shell inside the container for debugging
docker-shell:
	docker compose run --rm --entrypoint bash balebos

# Remove all Docker volumes and images related to balebos
docker-clean:
	docker compose down -v
	docker rmi $(DOCKER_IMAGE)

## --- Help ---

help:
	@echo "Balebos Management CLI"
	@echo ""
	@echo "Local Commands:"
	@echo "  make build          - Build release binary"
	@echo "  make run-ui         - Run interactive TUI"
	@echo "  make list           - List assets in DuckDB"
	@echo "  make scan domain=X  - Scan a single domain"
	@echo "  make clean          - Remove build files and local DuckDB"
	@echo ""
	@echo "Dev & Quality:"
	@echo "  make dev-check      - Quick compilation check"
	@echo "  make dev-test       - Run unit and integration tests"
	@echo "  make dev-fix        - Run rustfmt and clippy fix"
	@echo "  make dev-doc        - Generate and open documentation"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-build   - Build Docker image"
	@echo "  make docker-ui      - Run TUI in a container (recommmended)"
	@echo "  make docker-shell   - Enter container shell for debugging"
	@echo "  make docker-clean   - Remove container and volumes"
