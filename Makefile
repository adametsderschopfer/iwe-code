.DEFAULT_GOAL := check

PYTHON ?= python3

.PHONY: run build release test fmt fmt-check lint typecheck security check preflight platform-check

run:
	cargo run --locked

build:
	cargo build --locked

release:
	cargo build --locked --release

test:
	cargo test --locked --all-targets --all-features

fmt:
	cargo fmt --all
	dprint fmt

fmt-check:
	cargo fmt --all -- --check
	dprint check

lint:
	cargo clippy --locked --all-targets --all-features -- -D warnings

typecheck:
	cargo check --locked --all-targets --all-features

security:
	cargo audit --file Cargo.lock

check: fmt-check lint typecheck test security

preflight:
	$(MAKE) check
	$(PYTHON) -m unittest discover -s .github/scripts/tests
	actionlint
	$(MAKE) build

platform-check:
	$(PYTHON) .github/scripts/local-platform-checks.py
