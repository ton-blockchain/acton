mod dedust;
mod jettons;
mod pton;
mod stonfi;

use super::enrichment::{ActionInfoBox, EnrichmentContext};
use super::{Action, ActionProvider};

pub(in crate::actions) fn providers() -> &'static [&'static dyn ActionProvider] {
    &[
        &stonfi::StonfiProvider,
        &dedust::DedustProvider,
        &pton::PtonProvider,
        &jettons::JettonsProvider,
    ]
}

pub(in crate::actions) fn describe_action(
    action: &Action,
    ctx: &EnrichmentContext<'_>,
) -> Option<ActionInfoBox> {
    providers()
        .iter()
        .find_map(|provider| provider.describe(action, ctx))
}
