//! Ledger identity checks, performed before any scheduling side effect.
use crate::{
    build_id::{self, Verdict},
    dispatch::Ctx,
    store::StoreError,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OlderKernel {
    pub message: String,
    pub kernel: String,
    pub ledger: String,
}

fn rejection(ctx: &Ctx, ledger: String) -> OlderKernel {
    OlderKernel {
        message: build_id::older_kernel_message(&ctx.build_id, &ledger),
        kernel: ctx.build_id.clone(),
        ledger,
    }
}

pub fn check(ctx: &Ctx) -> Result<Option<OlderKernel>, StoreError> {
    Ok(check_recorded(ctx, ctx.store.kernel_mark()?))
}

pub(crate) fn check_recorded(ctx: &Ctx, ledger: Option<String>) -> Option<OlderKernel> {
    if ctx.allow_older_kernel && !ctx.adopted_older_kernel {
        return None;
    }
    match build_id::verdict(&ctx.build_id, ledger.as_deref()) {
        Verdict::Older | Verdict::Incomparable => ledger.map(|m| rejection(ctx, m)),
        _ => None,
    }
}

pub fn claim(ctx: &mut Ctx, method: &str) -> Result<Option<OlderKernel>, StoreError> {
    let adopt = ctx.allow_older_kernel && !ctx.adopted_older_kernel;
    let (rejected, adopted) = ctx
        .store
        .claim_kernel_mark_with_adoption(&ctx.build_id, adopt.then_some(method))?;
    ctx.adopted_older_kernel |= adopted;
    Ok(rejected.map(|m| rejection(ctx, m)))
}
