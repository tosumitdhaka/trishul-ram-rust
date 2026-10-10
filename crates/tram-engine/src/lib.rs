// SPDX-License-Identifier: Apache-2.0
//! P1 ephemeral-only primitives. Filesystem admission is deliberately absent.
pub const EPHEMERAL_ONLY: bool = true;
pub mod budget;
pub mod codec;
pub mod transform;
pub mod obligations;
pub mod harness;
