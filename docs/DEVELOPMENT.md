# Development

## Toolchain

- **Rust**: `1.85.0` (from `rust-toolchain.toml`)
- **Components**: `rustfmt`, `clippy`, `rust-src`, `llvm-tools-preview`

## Build & Check

```bash
make check              # cargo check --workspace
make build              # cargo build --workspace
make fmt                # cargo fmt --all
make fmt-check          # format check only
make clippy             # clippy on first-party crates, -D warnings
```

## Testing

```bash
make test               # unit + integration tests (first-party crates only)
make test-vm-backend    # runtime + stdlib with real Move VM
make test-cross-repo    # wire-format compatibility assertions
make test-verified-compile  # package builds with bytecode verification
make test-native-compile    # package builds with move-compiler-v2
make test-all-features  # all of the above in sequence
```

Script-driven validation:

```bash
make smoke-offline-build   # offline build path + artifact layout assertions
make validate-compat       # cross-workspace compatibility sweep
```

## Workspace Model

- **Default members**: the 5 `nexus-move-*` crates — these are the primary validation targets.
- **Vendor members**: 20 upstream Move crates under `vendor/` — workspace members for explicit dependency edges, but not included in default `make test`/`make clippy` targets.

## rust-analyzer

`.vscode/settings.json` restricts rust-analyzer checks to the 5 first-party crates, suppressing diagnostics from vendored crates with stripped dev-dependencies.

## Example Artifacts

`examples/counter/nexus-artifact/` contains committed build output used by the offline build smoke test. These are test fixtures, not incidental build leftovers.

## Vendor Crate Policy

Only touch `vendor/` when:
- A security or compatibility fix is required
- A first-party crate needs an upstream capability in scope
- Deliberate dependency narrowing is documented

All routine development stays within the 5 first-party crates.