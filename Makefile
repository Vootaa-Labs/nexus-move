# nexus-move development workflow

# Nexus crates (excludes vendored Move crates whose test targets need proptest)
NEXUS_PKGS := -p nexus-move-types -p nexus-move-bytecode -p nexus-move-stdlib \
              -p nexus-move-runtime -p nexus-move-package

.PHONY: all build check fmt fmt-check clippy test clean smoke-offline-build audit-deps vendor-audit \
       test-native-compile check-native-compile compile-nursery validate-compat test-cross-repo help

all: fmt-check clippy test

build:
	cargo build --workspace

check:
	cargo check --workspace

check-vm-backend:
	cargo check -p nexus-move-runtime --features vm-backend

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy $(NEXUS_PKGS) -- -D warnings

test:
	cargo test $(NEXUS_PKGS)

test-bootstrap-backend:
	cargo test $(NEXUS_PKGS) --features bootstrap-vendor

test-vm-backend:
	cargo test -p nexus-move-runtime -p nexus-move-stdlib --features vm-backend

test-verified-compile:
	cargo test -p nexus-move-package --features verified-compile

check-verified-compile:
	cargo check -p nexus-move-package --features verified-compile

test-native-compile:
	cargo test -p nexus-move-package --features native-compile

check-native-compile:
	cargo check -p nexus-move-package --features native-compile

compile-nursery:
	cargo test -p nexus-move-package --features native-compile -- native_backend::tests::generate_nursery_bytecode --exact --ignored

test-all-features:
	cargo test $(NEXUS_PKGS)
	cargo test -p nexus-move-stdlib --features vm-backend
	cargo test -p nexus-move-runtime --features vm-backend
	cargo test -p nexus-move-package --features verified-compile
	cargo test -p nexus-move-package --features native-compile

smoke-offline-build:
	./scripts/check-offline-build.sh

audit-deps:
	@echo "dependency freeze audit is defined in docs/DEPENDENCY_FREEZE.md"

vendor-audit:
	./scripts/audit-move-workspace-inheritance.sh

validate-compat:
	./scripts/validate-main-repo-compat.sh

test-cross-repo:
	cargo test -p nexus-move-runtime --features vm-backend --test cross_repo_compat

clean:
	cargo clean

help:
	@echo "nexus-move commands:"
	@echo "  make build               Build workspace (all crates)"
	@echo "  make check               Fast workspace type-check"
	@echo "  make check-vm-backend    Type-check with real Move VM backend"
	@echo "  make fmt                 Format all code"
	@echo "  make fmt-check           Check formatting only"
	@echo "  make clippy              Run clippy with -D warnings"
	@echo "  make test                Run workspace tests"
	@echo "  make test-bootstrap-backend  Run with bootstrap move-package backend"
	@echo "  make test-vm-backend     Run with real Move VM backend integration tests"
	@echo "  make test-verified-compile  Run with verified bytecode compile backend"
	@echo "  make check-verified-compile Type-check verified compile backend"
	@echo "  make test-native-compile Run with native move-compiler-v2 backend"
	@echo "  make check-native-compile Type-check native compile backend"
	@echo "  make compile-nursery     Recompile nursery modules (guid, event) to .mv"
	@echo "  make test-all-features   Run all feature-gated tests in sequence"
	@echo "  make smoke-offline-build Run offline build smoke check"
	@echo "  make validate-compat     Validate cross-workspace main repo compatibility"
	@echo "  make test-cross-repo     Run cross-repo compatibility tests"
	@echo "  make audit-deps          Print dependency freeze reminder"
	@echo "  make vendor-audit        Inspect upstream Move workspace inheritance blockers"
	@echo "  make clean               Remove build artifacts"
