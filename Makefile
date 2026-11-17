SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

ENV_FILE ?= .env

.PHONY: help setup build build-debug run dev check test lint fmt fmt-check clean

help:
	@printf '%s\n' \
	  'make setup       Create .env from .env.example if missing' \
	  'make build       Build botd and botctl for release' \
	  'make run         Build and run with .env' \
	  'make dev         Build and run in debug mode with .env' \
	  'make check       Check all Rust targets' \
	  'make test        Run workspace tests' \
	  'make lint        Run Clippy' \
	  'make fmt         Format Rust code' \
	  'make fmt-check   Check Rust formatting' \
	  'make clean       Remove build artifacts' \
	  'Override the environment file with ENV_FILE=<path>.'

setup:
	@if [ -e "$(ENV_FILE)" ]; then \
	  printf '%s\n' 'Environment file already exists; keeping it.'; \
	else \
	  (umask 077; cp .env.example "$(ENV_FILE)"); \
	  printf '%s\n' 'Environment file created. Edit it before running.'; \
	fi

build:
	cargo build --workspace --release

build-debug:
	cargo build --workspace

# Source the file in the shell, never in Make: values may contain spaces or $.
# Build both binaries so botctl is available beside botd during agent runs.
run: build
	@set -a; \
	if [ -f "$(ENV_FILE)" ]; then . "$(abspath $(ENV_FILE))"; fi; \
	set +a; \
	exec cargo run --release --bin botd

dev: build-debug
	@set -a; \
	if [ -f "$(ENV_FILE)" ]; then . "$(abspath $(ENV_FILE))"; fi; \
	set +a; \
	exec cargo run --bin botd

check:
	cargo check --workspace --all-targets

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clean:
	cargo clean
