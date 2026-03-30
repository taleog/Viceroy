SHELL := /bin/bash

CARGO ?= cargo
PYTHON ?= python3

PACKAGE := viceroy
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
BUILD_DIR := target
RELEASE_BIN := $(BUILD_DIR)/release/$(PACKAGE)

UPDATER_PORT ?= 8999
MOCK_METADATA_URL := http://127.0.0.1:$(UPDATER_PORT)/latest.json
MOCK_SERVER_LOG ?= /tmp/viceroy-mock-server.log

.DEFAULT_GOAL := help

.PHONY: help setup fmt lint test test-updater run dev dev-watch release app install-app clean mock-server mock-update-check mock-e2e check version bump-version

help:
	@echo "Viceroy Development Commands (v$(VERSION))"
	@echo ""
	@echo "Setup & Development:"
	@echo "  make setup                                 # Set up development environment (git hooks, etc.)"
	@echo "  make run RUN_ARGS='--silent-update-check'  # Run Viceroy with optional CLI args"
	@echo "  make dev                                  # Fast debug run via ./dev_open.sh"
	@echo "  make dev-watch                            # Auto-rebuild and restart on changes"
	@echo "  make fmt                                   # Format Rust sources"
	@echo "  make lint                                  # Run cargo clippy with -D warnings"
	@echo "  make test                                  # Run the full test suite"
	@echo "  make check                                 # Run fmt + lint + test"
	@echo ""
	@echo "Build & Release:"
	@echo "  make release                               # Build target/release/$(PACKAGE)"
	@echo "  make app                                   # Build Viceroy.app in the repo root"
	@echo "  make install-app                           # Build, copy to /Applications, and open it"
	@echo "  make version                               # Show current version"
	@echo "  make clean                                 # Remove target artifacts"
	@echo ""
	@echo "Update System Testing:"
	@echo "  make mock-server                           # Serve release binary + metadata locally"
	@echo "  make mock-update-check                     # Run updater against the mock server URL"
	@echo "  make test-updater                          # Run the ignored updater integration test"
	@echo "  make mock-e2e                              # Build, launch mock server, run integration test"

setup:
	@echo "Setting up development environment..."
	@./scripts/setup-dev.sh

check: fmt lint test
	@echo "All checks passed!"

version:
	@echo "Viceroy version: $(VERSION)"

fmt:
	$(CARGO) fmt

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

test:
	$(CARGO) test

test-updater:
	VICEROY_UPDATE_METADATA_URL=$(MOCK_METADATA_URL) $(CARGO) test --test updater_integration -- --ignored

run:
	$(CARGO) run $(if $(RUN_ARGS),-- $(RUN_ARGS))

dev:
	./dev_open.sh $(if $(RUN_ARGS),-- $(RUN_ARGS))

dev-watch:
	./dev_open.sh --watch $(if $(RUN_ARGS),-- $(RUN_ARGS))

$(RELEASE_BIN):
	$(CARGO) build --release

release: $(RELEASE_BIN)
	@echo "Release binary ready at $(RELEASE_BIN)"

app: $(RELEASE_BIN)
	./build_app.sh

install-app:
	./install_and_open_viceroy.sh

clean:
	$(CARGO) clean

mock-server: $(RELEASE_BIN)
	@echo "Serving $(RELEASE_BIN) via scripts/mock_update_server.py on $(MOCK_METADATA_URL)"
	$(PYTHON) scripts/mock_update_server.py --binary $(RELEASE_BIN) --version $(VERSION) --port $(UPDATER_PORT)

mock-update-check:
	@echo "Running updater against $(MOCK_METADATA_URL) (expects mock server to be running)"
	VICEROY_UPDATE_METADATA_URL=$(MOCK_METADATA_URL) $(CARGO) run -- --silent-update-check

mock-e2e: $(RELEASE_BIN)
	@echo "Starting mock server + updater integration test (logs -> $(MOCK_SERVER_LOG))"
	@set -euo pipefail; \
	$(PYTHON) scripts/mock_update_server.py --binary $(RELEASE_BIN) --version $(VERSION) --port $(UPDATER_PORT) > $(MOCK_SERVER_LOG) 2>&1 & \
	server_pid=$$!; \
	trap 'kill $$server_pid >/dev/null 2>&1 || true' EXIT; \
	sleep 1; \
	VICEROY_UPDATE_METADATA_URL=$(MOCK_METADATA_URL) $(CARGO) test --test updater_integration -- --ignored; \
	kill $$server_pid >/dev/null 2>&1 || true; \
	wait $$server_pid 2>/dev/null || true; \
	echo "Mock server stopped"
