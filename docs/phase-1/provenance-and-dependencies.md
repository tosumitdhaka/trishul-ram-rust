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

## Resolved Round 1 crate inventory (generated Rust 1.88 Cargo.lock)

The only added direct external dependency is `serde_yaml_ng = "=0.10.0"` (MIT; upstream [published manifest](https://docs.rs/crate/serde_yaml_ng/0.10.0/source/Cargo.toml.orig)), used exclusively by the pure plan compiler; its static dependencies are locked. The table below records **all 14 external direct/transitive packages** resolved by the hosted Rust 1.88 runner, version-locked in `Cargo.lock`. Source for every entry is `registry+https://github.com/rust-lang/crates.io-index`; the SHA-256 values are the corresponding Cargo registry checksums. License expressions come from the resolved `cargo metadata --locked` package metadata (run [37997593831](https://github.com/tosumitdhaka/trishul-ram-rust/actions/runs/37997593831), job `114047484987`). Local `tram-*` path crates use project Apache-2.0.

| Crate | Version | SPDX declared in Cargo metadata | Registry checksum (SHA-256) | License choice and notices |
|---|---|---|---|---|
| [equivalent 1.0.2](https://crates.io/crates/equivalent/1.0.2) | 1.0.2 | `Apache-2.0 OR MIT` | `877a4ace8713b0bcf2a4e7eec82529c029f1d0619886d18145fea96c3ffe5c0f` | Apache-2.0 option; preserve attribution/NOTICE |
| [hashbrown 0.17.1](https://crates.io/crates/hashbrown/0.17.1) | 0.17.1 | `MIT OR Apache-2.0` | `ed5909b6e89a2db4456e54cd5f673791d7eca6732202bbf2a9cc504fe2f9b84a` | Apache-2.0 option; preserve attribution/NOTICE |
| [indexmap 2.14.2](https://crates.io/crates/indexmap/2.14.2) | 2.14.2 | `Apache-2.0 OR MIT` | `cc4e190f5d26ca7051642629da2c52fc03bde85a03197c99408dcd291734c855` | Apache-2.0 option; preserve attribution/NOTICE |
| [itoa 1.0.18](https://crates.io/crates/itoa/1.0.18) | 1.0.18 | `MIT OR Apache-2.0` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` | Apache-2.0 option; preserve attribution/NOTICE |
| [proc-macro2 1.0.107](https://crates.io/crates/proc-macro2/1.0.107) | 1.0.107 | `MIT OR Apache-2.0` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` | Apache-2.0 option; preserve attribution/NOTICE |
| [quote 1.0.47](https://crates.io/crates/quote/1.0.47) | 1.0.47 | `MIT OR Apache-2.0` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` | Apache-2.0 option; preserve attribution/NOTICE |
| [ryu 1.0.23](https://crates.io/crates/ryu/1.0.23) | 1.0.23 | `Apache-2.0 OR BSL-1.0` | `9774ba4a74de5f7b1c1451ed6cd5285a32eddb5cccb8cc655a4e50009e06477f` | Apache-2.0 option; preserve attribution/NOTICE |
| [serde 1.0.229](https://crates.io/crates/serde/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` | Apache-2.0 option; preserve attribution/NOTICE |
| [serde_core 1.0.229](https://crates.io/crates/serde_core/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` | Apache-2.0 option; preserve attribution/NOTICE |
| [serde_derive 1.0.229](https://crates.io/crates/serde_derive/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` | Apache-2.0 option; preserve attribution/NOTICE |
| [serde_yaml_ng 0.10.0](https://crates.io/crates/serde_yaml_ng/0.10.0) | 0.10.0 | `MIT` | `7b4db627b98b36d4203a7b458cf3573730f2bb591b28871d916dfa9efabfd41f` | MIT; preserve notice |
| [syn 3.0.6](https://crates.io/crates/syn/3.0.6) | 3.0.6 | `MIT OR Apache-2.0` | `8593e8e72159ed2257d083c7a454a85cbf854f37a0966d8d483aff8c8a3ebcee` | Apache-2.0 option; preserve attribution/NOTICE |
| [unicode-ident 1.0.26](https://crates.io/crates/unicode-ident/1.0.26) | 1.0.26 | `(MIT OR Apache-2.0) AND Unicode-3.0` | `d245f478577f809a851594d02313b640fb437e0bb33866753cff937863096954` | Apache-2.0 **and** Unicode-3.0; preserve dual notices |
| [unsafe-libyaml 0.2.11](https://crates.io/crates/unsafe-libyaml/0.2.11) | 0.2.11 | `MIT` | `673aac59facbab8a9007c7f6108d11f63b603f7cabff99fabf650fea5c32b861` | MIT; preserve notice |

### License / redistribution review disposition

- **Review evidence and choice:** reviewed direct package manifest and complete resolved SPDX expressions against the Apache-2.0 target. The permissive expressions above provide Apache-2.0 alternatives where dual-licensed; MIT-only packages require preservation of MIT copyright/license notices; `unicode-ident` has an additional conjunctive **Unicode-3.0** obligation; `ryu` may be selected under Apache-2.0 rather than BSL-1.0. No copyleft-only or unknown-expression package appears in this pinned graph.
- **Source/provenance:** exact registry source+version+checksum is locked and checksummed; independent crate source content has not been vendored or imported. Rust source authored here is original and declares Apache-2.0. The pinned Python and SNMP repositories are reference context only.
- **Notice gate for redistributable binaries:** retain third-party copyright/LICENSE/NOTICE texts from the exact packages in the release attribution manifest when distributing binaries; verify full upstream license files against declared SPDX before production packaging. No P1 runtime/release is authorized. Potential security advisories and repository-wide legal/compliance approval remain separate gates; metadata SPDX expressions alone are not a legal conclusion.
- **Status:** preliminary provenance/inventory completeness on exact lock graph; **formal dependency license/notice approval remains pending independent review**. Do not misstate this self-authored inventory as external legal certification or production release readiness.
