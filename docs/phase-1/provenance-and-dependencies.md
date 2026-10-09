# Phase 1 code provenance, SPDX and dependency policy

Status: Round 1 implementation prerequisite. License selection is governed by the frozen Phase 0 decision register at `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. This policy creates **no** authorization to copy reference implementations or third-party assets.

## Repository and source files

- The original Rust implementation in this repository is licensed under **Apache-2.0**; see root `LICENSE`. This statement does not relicense upstream dependencies, imported assets, or third-party material.
- Each newly authored `.rs` source file must start with `// SPDX-License-Identifier: Apache-2.0`. Add a copyright notice only when ownership is verified; do not assert ownership of upstream code.
- Rust manifests, YAML and other newly authored text supporting comments should use `# SPDX-License-Identifier: Apache-2.0` where it is syntactically supported. JSON and `Cargo.lock` are excluded from comment-header requirements. The root Apache license is the canonical license text.
- Retain upstream copyright, SPDX identifiers, license/NOTICE files, required attributions and redistribution terms for any individually approved imported file. Record exceptions before import.

## Code and asset import gates

No Python implementation code, JavaScript/UI assets, compiled or source MIB bundles, fixture from the Python reference, generated vendor file, `trishul-snmp` implementation, or other third-party implementation may be copied without an **individual provenance review** recording exact repository URL/revision/path, original author/rights holder, license text and SPDX, modification status, required notices, redistribution obligations, and an approval decision. A repository's top-level license alone does not establish permission for every file. No automatic translation/copying of Python source. The four freshly authored and Phase 0-pinned `docs/phase-0/fixtures/p1/*` files are existing frozen design artifacts; do not modify them. Do not vendor libraries without a separate explicit gate.

## Dependency review and reproducibility

Every direct or transitive third-party crate admitted after this prerequisite needs an inventory entry with **crate + version, registry/git source and immutable revision/checksum, SPDX license expression, attribution/NOTICE needs, redistribution conditions, license evidence URL, reviewer, and approval**. Inventory must be refreshed whenever `Cargo.lock` changes. Reject missing, ambiguous or incompatible licensing; reject license expressions before positive verification, rather than assuming that `OR` alternatives are acceptable. `trishul-snmp` is reserved for a later phase, and any future pinned Git revision must be reviewed as a dependency, not copied.

- Initial dependency inventory at this documentation-only commit: **none** (no `Cargo.toml`, `Cargo.lock`, `.rs` files or dependencies introduced).
- Prefer Rust standard library for foundational types; minimize dependencies and surface areas. Registry dependencies must use exact reviewed versions or constrained manifests plus a **committed, reproducible Cargo.lock** for binary/test builds. Git dependencies, if later approved, must pin immutable revisions; floating branch/tag HEADs are forbidden.
- Pin an explicit Rust toolchain (exact `rust-toolchain.toml`, or a documented MSRV + validated CI version matrix). Validate with `cargo metadata --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Use `--locked` for reproducible CI compilation and fail if lockfile resolution would change.
- A dependency or toolchain update requires an explicit manifest + lockfile diff, revised inventory (including transitive crates), security/license review, and exact-SHA build/test evidence. No unstated license exception, network dependency in offline tests or unreviewed code generation.

## Current Round 1 assurance

This documentation gate does **not** imply the Rust foundation exists, that any tests ran, that the Python comparison passed, or that licenses of not-yet-selected crates have been reviewed. Round 1 implementation and source review must separately establish those facts. Frozen `docs/phase-0/**` and fixture blobs are immutable.
