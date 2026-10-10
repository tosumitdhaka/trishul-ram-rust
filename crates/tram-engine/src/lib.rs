// SPDX-License-Identifier: Apache-2.0
//! P1 ephemeral-only primitives. Effects are permitted only through the trusted-root TestHarness.
pub const EPHEMERAL_ONLY: bool = true;
pub mod budget;
pub mod codec;
pub mod harness;
pub mod obligations;
pub mod transform;
