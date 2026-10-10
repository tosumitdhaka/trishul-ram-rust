// SPDX-License-Identifier: Apache-2.0
//! P1 ephemeral-only primitives. Filesystem admission is deliberately absent.
pub const EPHEMERAL_ONLY: bool = true;
pub mod budget;
pub mod transform;
