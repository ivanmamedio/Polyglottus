# Polyglottus - Development Commands
# Usage: just <command>
# See: https://github.com/casey/just

# Variables
CARGO := cargo
RUSTUP := rustup
PNPM := pnpm
TAURI := cargo tauri

# Default target
default: help

# Help
help:
	@echo "Polyglottus Development Commands"
	@echo ""
	@echo "Setup:"
	@echo "  just setup           - Install dependencies and setup project"
	@echo "  just setup-rust      - Install Rust toolchain"
	@echo "  just setup-node      - Install Node.js dependencies"
	@echo ""
	@echo "Development:"
	@echo "  just check           - Run cargo check (fast type check)"
	@echo "  just lint            - Run clippy and fmt checks"
	@echo "  just fmt             - Format code with rustfmt"
	@echo "  just test            - Run all tests"
	@echo "  just test-unit       - Run unit tests only"
	@echo "  just test-integration - Run integration tests"
	@echo "  just run-cli         - Run CLI binary"
	@echo "  just run-desktop     - Run Tauri desktop app (dev mode)"
	@echo ""
	@echo "Building:"
	@echo "  just build           - Build all crates (release)"
	@echo "  just build-cli       - Build CLI binary (release)"
	@echo "  just build-desktop   - Build Tauri desktop app (release)"
	@echo "  just package-flatpak - Package Flatpak (Linux)"
	@echo "  just package-windows - Package Windows installer"
	@echo ""
	@echo "Quality:"
	@echo "  just audit           - Run cargo audit (security)"
	@echo "  just deny            - Run cargo deny (licenses, bans, sources)"
	@echo "  just doc             - Generate documentation"
	@echo "  just clean           - Clean build artifacts"
	@echo ""
	@echo "CI:"
	@echo "  just ci              - Run full CI pipeline locally"
	@echo "  just pre-commit      - Run pre-commit checks"

# =============================================================================
# SETUP
# =============================================================================

setup: setup-rust setup-node

setup-rust:
	@echo "Installing Rust toolchain..."
	@{{RUSTUP}} toolchain install stable --component rustfmt clippy rust-src
	@{{RUSTUP}} target add x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
	@{{CARGO}} install cargo-deny cargo-audit cargo-nextest cargo-insta cargo-llvm-cov just

setup-node:
	@echo "Installing Node.js dependencies..."
	@cd apps/desktop/ui && {{PNPM}} install

# =============================================================================
# DEVELOPMENT
# =============================================================================

check:
	@{{CARGO}} check --workspace --all-targets

lint: fmt clippy

fmt:
	@{{CARGO}} fmt --all --check

clippy:
	@{{CARGO}} clippy --workspace --all-targets -- -D warnings

test:
	@{{CARGO}} nextest run --workspace

test-unit:
	@{{CARGO}} nextest run --workspace --lib

test-integration:
	@{{CARGO}} nextest run --workspace --test integration

run-cli:
	@{{CARGO}} run --bin pgl-cli -- --help

run-desktop:
	@cd apps/desktop && {{TAURI}} dev

# =============================================================================
# BUILDING
# =============================================================================

build:
	@{{CARGO}} build --workspace --release

build-cli:
	@{{CARGO}} build --bin pgl-cli --release

build-desktop:
	@cd apps/desktop && {{TAURI}} build

package-flatpak:
	@echo "Packaging Flatpak..."
	@cd packaging/flatpak && flatpak-builder --force-clean build-dir org.polyglottus.Polyglottus.yml

package-windows:
	@echo "Packaging Windows installer..."
	@cd apps/desktop && {{TAURI}} build --target x86_64-pc-windows-msvc

# =============================================================================
# QUALITY
# =============================================================================

audit:
	@{{CARGO}} audit

deny:
	@{{CARGO}} deny check

doc:
	@{{CARGO}} doc --workspace --no-deps --document-private-items

clean:
	@{{CARGO}} clean
	@cd apps/desktop/ui && {{PNPM}} run clean 2>/dev/null || true

# =============================================================================
# CI PIPELINE
# =============================================================================

ci: lint test deny audit doc
	@echo "✅ All CI checks passed!"

pre-commit: fmt clippy test-unit
	@echo "✅ Pre-commit checks passed!"

# =============================================================================
# UTILITIES
# =============================================================================

update-deps:
	@{{CARGO}} update --workspace
	@cd apps/desktop/ui && {{PNPM}} update

outdated:
	@{{CARGO}} outdated --workspace --depth 1 2>/dev/null || echo "cargo-outdated not installed"

tree:
	@{{CARGO}} tree --workspace --depth 2

bench:
	@{{CARGO}} bench --workspace

coverage:
	@{{CARGO}} llvm-cov --workspace --html --open

# Generate types for Tauri IPC
generate-types:
	@cd apps/desktop && {{TAURI}} generate-types

# Install Git hooks
install-hooks:
	@echo "#!/bin/sh\njust pre-commit" > .git/hooks/pre-commit
	@chmod +x .git/hooks/pre-commit
	@echo "Git hooks installed!"

.PHONY: help setup setup-rust setup-node check lint fmt clippy test test-unit test-integration run-cli run-desktop build build-cli build-desktop package-flatpak package-windows audit deny doc clean ci pre-commit update-deps outdated tree bench coverage generate-types install-hooks