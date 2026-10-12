//! Named one-off mechanics that don't fit the effect primitives.

use crate::combat::{Ctx, Exec};

pub(crate) fn run(_ex: &mut Exec, _ctx: &mut Ctx, name: &str) {
    match name {
        _ => debug_assert!(false, "unknown hook `{name}`"),
    }
}
