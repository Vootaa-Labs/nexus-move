# Release

## Versioning

- Follows SemVer: `MAJOR.MINOR.PATCH`
- Workspace version in root `Cargo.toml` → `[workspace.package] version`
- Git tag format: `v0.1.1`
- Consumer pin: `{ git = "https://github.com/vootaa-labs/nexus-move", tag = "v0.1.1" }`

## Pre-Release Checklist

```bash
make fmt-check
make clippy
make test
make test-vm-backend
make test-cross-repo
make check-verified-compile
make check-native-compile
make smoke-offline-build
```

## Repository Must Include

- Committed `Cargo.lock`
- CI workflow under `.github/workflows/`
- Embedded stdlib bytecodes and example artifact fixtures
- Up-to-date `docs/` documentation

## Tagging

```bash
git tag v<VERSION>
git push origin v<VERSION>
```

Consumers reference the tag in their `Cargo.toml` git dependency.

## Release Notes Should Cover

- API changes in the 5 first-party crates
- Vendor freeze updates (upstream commit, rationale)
- Stdlib module inventory changes
- Feature flag changes
- Wire-format or artifact-format breaking changes