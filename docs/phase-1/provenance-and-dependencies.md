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

## Round 2 Linux capability harness / SHA-256 dependency review (candidate)

**Declared direct dependencies:** `cap-std = "=3.4.5"` (capability-rooted file operations) and `sha2 = "=0.10.9"` (SHA-256 source-unit fingerprints). **Rust toolchain:** 1.88.0. They are exclusively for the P1 disposable-root test harness, not network or production delivery. This update does not import code from either upstream project; only Cargo resolves the immutable registry distributions. Direct upstream source/declared licenses: [cap-std 3.4.5 manifest](https://docs.rs/crate/cap-std/3.4.5/source/Cargo.toml.orig) (`Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`), [sha2 0.10.9](https://crates.io/crates/sha2/0.10.9) (`MIT OR Apache-2.0`). The resolver may select a newer compatible **transitive** `cap-primitives`; this is separately identified below.

**Complete Cargo.lock inventory:** 51 registry packages (Linux and Windows-target dependencies); all sources are `registry+https://github.com/rust-lang/crates.io-index` with the exact checksum shown. Versions, license expressions and registry sources were extracted from pinned [GitHub Actions job 114132656203](https://github.com/tosumitdhaka/trishul-ram-rust/actions/runs/38024607085) `cargo metadata --locked` after generating Cargo.lock with Rust 1.88.0. Checksum/identity was read from the committed generated lockfile. No floating Git deps or vendored source. All five workspace crates are project Apache-2.0.

| Locked crate | Version | Cargo SPDX metadata | Registry checksum (SHA256) |
|---|---|---|---|
| [ambient-authority](https://crates.io/crates/ambient-authority/0.0.2) | 0.0.2 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `e9d4ee0d472d1cd2e28c97dfa124b3d8d992e10eb0a035f33f5d12e3a177ba3b` |
| [bitflags](https://crates.io/crates/bitflags/2.13.2) | 2.13.2 | `MIT OR Apache-2.0` | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| [block-buffer](https://crates.io/crates/block-buffer/0.10.4) | 0.10.4 | `MIT OR Apache-2.0` | `3078c7629b62d3f0439517fa394996acacc5cbc91c5a20d8c658e77abd503a71` |
| [cap-primitives](https://crates.io/crates/cap-primitives/3.4.6) | 3.4.6 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `8e0bf07d379916947be6c4a07f43684153d710a2896c31f9e97781362895596c` |
| [cap-std](https://crates.io/crates/cap-std/3.4.5) | 3.4.5 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `b6dc3090992a735d23219de5c204927163d922f42f575a0189b005c62d37549a` |
| [cfg-if](https://crates.io/crates/cfg-if/1.0.5) | 1.0.5 | `MIT OR Apache-2.0` | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| [cpufeatures](https://crates.io/crates/cpufeatures/0.2.17) | 0.2.17 | `MIT OR Apache-2.0` | `59ed5838eebb26a2bb2e58f6d5b5316989ae9d08bab10e0e6d103e656d1b0280` |
| [crypto-common](https://crates.io/crates/crypto-common/0.1.7) | 0.1.7 | `MIT OR Apache-2.0` | `78c8292055d1c1df0cce5d180393dc8cce0abec0a7102adb6c7b1eef6016d60a` |
| [digest](https://crates.io/crates/digest/0.10.7) | 0.10.7 | `MIT OR Apache-2.0` | `9ed9a281f7bc9b7576e61468ba615a66a5c8cfdff42420a70aa82701a3b1e292` |
| [equivalent](https://crates.io/crates/equivalent/1.0.2) | 1.0.2 | `Apache-2.0 OR MIT` | `877a4ace8713b0bcf2a4e7eec82529c029f1d0619886d18145fea96c3ffe5c0f` |
| [errno](https://crates.io/crates/errno/0.3.14) | 0.3.14 | `MIT OR Apache-2.0` | `39cab71617ae0d63f51a36d69f866391735b51691dbda63cf6f96d042b63efeb` |
| [fs-set-times](https://crates.io/crates/fs-set-times/0.20.3) | 0.20.3 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `94e7099f6313ecacbe1256e8ff9d617b75d1bcb16a6fddef94866d225a01a14a` |
| [generic-array](https://crates.io/crates/generic-array/0.14.7) | 0.14.7 | `MIT` | `85649ca51fd72272d7821adaf274ad91c288277713d9c18820d8499a7ff69e9a` |
| [hashbrown](https://crates.io/crates/hashbrown/0.17.1) | 0.17.1 | `MIT OR Apache-2.0` | `ed5909b6e89a2db4456e54cd5f673791d7eca6732202bbf2a9cc504fe2f9b84a` |
| [indexmap](https://crates.io/crates/indexmap/2.14.2) | 2.14.2 | `Apache-2.0 OR MIT` | `cc4e190f5d26ca7051642629da2c52fc03bde85a03197c99408dcd291734c855` |
| [io-extras](https://crates.io/crates/io-extras/0.18.4) | 0.18.4 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `2285ddfe3054097ef4b2fe909ef8c3bcd1ea52a8f0d274416caebeef39f04a65` |
| [io-lifetimes](https://crates.io/crates/io-lifetimes/2.0.4) | 2.0.4 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `06432fb54d3be7964ecd3649233cddf80db2832f47fec34c01f65b3d9d774983` |
| [ipnet](https://crates.io/crates/ipnet/2.12.2) | 2.12.2 | `MIT OR Apache-2.0` | `791930b43c0d5973160d90a8f3894509f2b273430f5c5c73b668636d0287c5c0` |
| [itoa](https://crates.io/crates/itoa/1.0.18) | 1.0.18 | `MIT OR Apache-2.0` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| [libc](https://crates.io/crates/libc/0.2.190) | 0.2.190 | `MIT OR Apache-2.0` | `ce5d3ddc6d3fa000eb1536d85e147bfe31aacaba692ed6a876f95cb7c855be78` |
| [linux-raw-sys](https://crates.io/crates/linux-raw-sys/0.12.1) | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `32a66949e030da00e8c7d4434b251670a91556f4144941d37452769c25d58a53` |
| [maybe-owned](https://crates.io/crates/maybe-owned/0.3.4) | 0.3.4 | `MIT OR Apache-2.0` | `4facc753ae494aeb6e3c22f839b158aebd4f9270f55cd3c79906c45476c47ab4` |
| [once_cell](https://crates.io/crates/once_cell/1.21.4) | 1.21.4 | `MIT OR Apache-2.0` | `9f7c3e4beb33f85d45ae3e3a1792185706c8e16d043238c593331cc7cd313b50` |
| [proc-macro2](https://crates.io/crates/proc-macro2/1.0.107) | 1.0.107 | `MIT OR Apache-2.0` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| [quote](https://crates.io/crates/quote/1.0.47) | 1.0.47 | `MIT OR Apache-2.0` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| [rustix](https://crates.io/crates/rustix/1.1.5) | 1.1.5 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `891efababe418670775f199f0d233d84843c227a0949a883ce15b37c78d6629d` |
| [rustix-linux-procfs](https://crates.io/crates/rustix-linux-procfs/0.1.1) | 0.1.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | `2fc84bf7e9aa16c4f2c758f27412dc9841341e16aa682d9c7ac308fe3ee12056` |
| [ryu](https://crates.io/crates/ryu/1.0.23) | 1.0.23 | `Apache-2.0 OR BSL-1.0` | `9774ba4a74de5f7b1c1451ed6cd5285a32eddb5cccb8cc655a4e50009e06477f` |
| [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` |
| [serde_core](https://crates.io/crates/serde_core/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| [serde_derive](https://crates.io/crates/serde_derive/1.0.229) | 1.0.229 | `MIT OR Apache-2.0` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| [serde_yaml_ng](https://crates.io/crates/serde_yaml_ng/0.10.0) | 0.10.0 | `MIT` | `7b4db627b98b36d4203a7b458cf3573730f2bb591b28871d916dfa9efabfd41f` |
| [sha2](https://crates.io/crates/sha2/0.10.9) | 0.10.9 | `MIT OR Apache-2.0` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` |
| [syn](https://crates.io/crates/syn/3.0.7) | 3.0.7 | `MIT OR Apache-2.0` | `d62a2e0561533f2ca2561d0cf27fd9fedb640a1bf2616ff5d5c80d99017faadc` |
| [typenum](https://crates.io/crates/typenum/1.20.1) | 1.20.1 | `MIT OR Apache-2.0` | `b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20` |
| [unicode-ident](https://crates.io/crates/unicode-ident/1.0.26) | 1.0.26 | `(MIT OR Apache-2.0) AND Unicode-3.0` | `d245f478577f809a851594d02313b640fb437e0bb33866753cff937863096954` |
| [unsafe-libyaml](https://crates.io/crates/unsafe-libyaml/0.2.11) | 0.2.11 | `MIT` | `673aac59facbab8a9007c7f6108d11f63b603f7cabff99fabf650fea5c32b861` |
| [version_check](https://crates.io/crates/version_check/0.9.5) | 0.9.5 | `MIT/Apache-2.0` | `0b928f33d975fc6ad9f86c8f283853ad26bdd5b10b7f1542aa2fa15e2289105a` |
| [windows-link](https://crates.io/crates/windows-link/0.2.1) | 0.2.1 | `MIT OR Apache-2.0` | `f0805222e57f7521d6a62e36fa9163bc891acd422f971defe97d64e70d0a4fe5` |
| [windows-sys](https://crates.io/crates/windows-sys/0.59.0) | 0.59.0 | `MIT OR Apache-2.0` | `1e38bc4d79ed67fd075bcc251a1c39b32a1776bbe92e5bef1f0bf1f8c531853b` |
| [windows-sys](https://crates.io/crates/windows-sys/0.61.2) | 0.61.2 | `MIT OR Apache-2.0` | `ae137229bcbd6cdf0f7b80a31df61766145077ddf49416a728b02cb3921ff3fc` |
| [windows-targets](https://crates.io/crates/windows-targets/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `9b724f72796e036ab90c1021d4780d4d3d648aca59e491e6b98e725b84e99973` |
| [windows_aarch64_gnullvm](https://crates.io/crates/windows_aarch64_gnullvm/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `32a4622180e7a0ec044bb555404c800bc9fd9ec262ec147edd5989ccd0c02cd3` |
| [windows_aarch64_msvc](https://crates.io/crates/windows_aarch64_msvc/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `09ec2a7bb152e2252b53fa7803150007879548bc709c039df7627cabbd05d469` |
| [windows_i686_gnu](https://crates.io/crates/windows_i686_gnu/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `8e9b5ad5ab802e97eb8e295ac6720e509ee4c243f69d781394014ebfe8bbfa0b` |
| [windows_i686_gnullvm](https://crates.io/crates/windows_i686_gnullvm/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `0eee52d38c090b3caa76c563b86c3a4bd71ef1a819287c19d586d7334ae8ed66` |
| [windows_i686_msvc](https://crates.io/crates/windows_i686_msvc/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `240948bc05c5e7c6dabba28bf89d89ffce3e303022809e73deaefe4f6ec56c66` |
| [windows_x86_64_gnu](https://crates.io/crates/windows_x86_64_gnu/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `147a5c80aabfbf0c7d901cb5895d1de30ef2907eb21fbbab29ca94c5b08b1a78` |
| [windows_x86_64_gnullvm](https://crates.io/crates/windows_x86_64_gnullvm/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `24d5b23dc417412679681396f2b49f3de8c1473deb516bd34410872eff51ed0d` |
| [windows_x86_64_msvc](https://crates.io/crates/windows_x86_64_msvc/0.52.6) | 0.52.6 | `MIT OR Apache-2.0` | `589f6da84c646204747d1270a2a5661ea66ed1cced2631d546fdfb155959f9ec` |
| [winx](https://crates.io/crates/winx/0.36.4) | 0.36.4 | `Apache-2.0 WITH LLVM-exception` | `3f3fd376f71958b862e7afb20cfe5a22830e1963462f3a17f49d82a6c1d1f42d` |

**License/notice gate:** Preserve exact upstream LICENSE/COPYRIGHT/NOTICE texts for redistributable artifacts. Particularly preserve `unicode-ident`'s conjunctive `Unicode-3.0`; `winx` has `Apache-2.0 WITH LLVM-exception`, and `version_check` declares legacy `MIT/Apache-2.0` metadata that requires manual SPDX normalization/evidence rather than pretending it is a valid modern SPDX expression. The LLVM exception text and OR-licensed choice need review before any redistribution. **This is an evidence inventory, not independent license or security clearance.** Outstanding action: inspect actual upstream license files/NOTICE for each resolved package and current advisories, obtain independent provenance approval before release. `Cargo.lock` and complete graph must be re-reviewed on any dependency change.
