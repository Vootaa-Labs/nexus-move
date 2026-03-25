# Release

## Repository Readiness Checklist

Before pushing changes intended for the public GitHub repository, run:

- `make fmt-check`
- `make clippy`
- `make test`
- `make test-vm-backend`
- `make test-cross-repo`
- `make check-verified-compile`
- `make check-native-compile`
- `make smoke-offline-build`

Run `make validate-compat` when validating the repository against the adjacent main Nexus repository layout.

## Publishing Expectations

The public repository should always contain:

- the committed `Cargo.lock`
- the current CI workflow under `.github/workflows/`
- the embedded stdlib and example artifact fixtures needed for offline validation
- the boundary and dependency documentation under `docs/`

## Pull Request Expectations

Changes are easier to review and safer to integrate when they stay within one of these buckets:

- first-party facade changes
- dependency freeze refreshes
- stdlib or package frontend updates
- CI, tooling, or documentation maintenance

Avoid mixing vendor refreshes with unrelated facade behavior changes unless the dependency update is the direct cause of the code change.

## Release Notes Guidance

When describing a release, call out:

- facade or API changes in the five first-party crates
- dependency freeze updates and upstream pin changes
- stdlib module inventory changes
- compile backend changes and feature-flag impacts
- compatibility or artifact format changes