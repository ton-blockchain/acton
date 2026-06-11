//! `DeDust` action matchers and renderers.
//!
//! `DeDust` docs describe the public TL-B surface:
//! - swaps and multi-hop routes: <https://docs.dedust.io/docs/swaps.md>
//! - message/event schemes: <https://docs.dedust.io/reference/tlb-schemes.md>
//!
//! A successful trace is split across contracts. The user sends funds to a Vault,
//! the Vault asks a Pool to swap, and the Pool sends a payout leg. Because of that
//! we first recognize protocol-internal legs, then compose them with the concrete
//! incoming offer and outgoing payout movements.

use super::super::enrichment::{
    ActionInfo, ActionInfoBox, EnrichmentContext, format_asset_amount, format_ton_amount,
    message_destination,
};
use super::super::{
    Action, ActionKind, ActionProvider, Asset, AssetAmount, BaseAction, BaseActionGraph,
    BaseActionKind, BaseMatch, BaseMatcher, CompositeMatch, CompositeMatcher, JettonTransferView,
    TraceNode, opcode_matches, opcodes,
};
use std::collections::BTreeSet;
use tycho_types::models::IntAddr;

pub(in crate::actions) struct DedustProvider;

impl ActionProvider for DedustProvider {
    fn base_matchers(&self) -> &'static [&'static dyn BaseMatcher] {
        &[
            &DedustNativeSwapLegMatcher,
            &DedustJettonSwapLegMatcher,
            &DedustPayoutMatcher,
        ]
    }

    fn composite_matchers(&self) -> &'static [&'static dyn CompositeMatcher] {
        &[&DedustSwapMatcher]
    }

    fn describe(&self, action: &Action, ctx: &EnrichmentContext<'_>) -> Option<ActionInfoBox> {
        match action.kind {
            ActionKind::DedustSwap => Some(Box::new(DedustSwapInfo {
                offer: offer_amount(action, ctx),
                ask: payout_amount(action, ctx),
            })),
            ActionKind::DedustPayout => {
                let root = ctx.root_fact(action)?;
                Some(Box::new(DedustPayoutInfo {
                    amount: root.coins("amount"),
                    destination: message_destination(root).cloned(),
                }))
            }
            _ => None,
        }
    }
}

struct DedustNativeSwapLegMatcher;

impl BaseMatcher for DedustNativeSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        // TON -> X starts at Native Vault `swap#ea06185d`, whose TL-B body contains
        // the TON `amount`. The Vault then enters the Pool through the pool-facing
        // `swap_external#61ee542d`, and the Pool emits a payout message for the
        // resulting asset. The optional `swap#9c610de3` Pool event is indexed when
        // present, but the actual user-facing asset movement is represented by the
        // payout branch.
        if !opcode_matches(root, opcodes::DEDUST_VAULT_NATIVE_V2_SWAP) {
            return None;
        }

        // Native Vault owns the TON input, but the actual price calculation happens
        // in the Pool. In traces this shows up as a Vault -> Pool hop named
        // `swap_external#61ee542d` in DeDust ABI schemas.
        let pool_swap = root.child(opcodes::DEDUST_POOL_V2_SWAP_EXTERNAL)?;

        // Once the Pool computes the output, it sends `pay_out_from_pool#ad4eb6f5`
        // toward the Vault/wallet branch that will deliver the ask asset.
        let payout = pool_swap.child(opcodes::DEDUST_POOL_V2_PAY_OUT_FROM_POOL)?;

        // DeDust also emits the public Pool event `swap#9c610de3` with asset_in,
        // asset_out, amount_in, and amount_out. The event is useful context when it
        // exists, but the user-facing action is still identified by the payout path,
        // so the event is optional for matching.
        let swap_event = pool_swap.child(opcodes::DEDUST_POOL_V2_SWAP_EVENT);

        let mut nodes = BTreeSet::from([root.id, pool_swap.id, payout.id]);
        if let Some(swap_event) = swap_event {
            nodes.insert(swap_event.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustNativeSwapLeg,
            nodes,
            root_node: root.id,
            user_facing: false,
        })
    }
}

struct DedustJettonSwapLegMatcher;

impl BaseMatcher for DedustJettonSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        // Jetton-funded swaps enter DeDust through a jetton transfer with a forward
        // payload (`swap#e3a0d482` in the Jetton Vault TL-B docs). In the action
        // graph that incoming jetton transfer is a separate base action, while the
        // DeDust-specific swap leg starts at the Pool `swap_external#61ee542d`.
        // Keeping this as a non-user-facing leg lets the composite matcher later
        // pair it with the resulting payout without duplicating the input transfer.
        if !opcode_matches(root, opcodes::DEDUST_POOL_V2_SWAP_EXTERNAL) {
            return None;
        }

        // From this point the shape is the same as native input: the Pool emits
        // `pay_out_from_pool#ad4eb6f5` to start the output branch.
        let payout = root.child(opcodes::DEDUST_POOL_V2_PAY_OUT_FROM_POOL)?;

        // The Pool event is optional for the same reason as in the native-input
        // matcher: it enriches the indexed node set when present, but the payout
        // branch is the structural proof that assets leave the Pool.
        let swap_event = root.child(opcodes::DEDUST_POOL_V2_SWAP_EVENT);

        let mut nodes = BTreeSet::from([root.id, payout.id]);
        if let Some(swap_event) = swap_event {
            nodes.insert(swap_event.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustJettonSwapLeg,
            nodes,
            root_node: root.id,
            user_facing: false,
        })
    }
}

struct DedustPayoutMatcher;

impl BaseMatcher for DedustPayoutMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        // Native TON output is delivered through Native Vault `payout#474f86cf`.
        // DeDust can attach protocol tail messages such as TON excesses or TON pay,
        // and those messages belong to the same payout leg.
        if !opcode_matches(root, opcodes::DEDUST_PAYOUT) {
            return None;
        }

        // `ton_excesses#37d3af9e` is an accounting side message that can be emitted
        // next to the payout. It belongs to the native payout leg, but it is not a
        // requirement for receiving TON.
        let ton_excesses = root.child(opcodes::DEDUST_TON_EXCESSES);

        // `dd_ton_pay#4c3e12d7` is the final TON payment helper message seen in
        // DeDust TON payout traces.
        let ton_pay = root.child(opcodes::DEDUST_TON_PAY);

        let mut nodes = BTreeSet::from([root.id]);
        if let Some(ton_excesses) = ton_excesses {
            nodes.insert(ton_excesses.id);
        }
        if let Some(ton_pay) = ton_pay {
            nodes.insert(ton_pay.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustPayout,
            nodes,
            root_node: root.id,
            user_facing: true,
        })
    }
}

struct DedustSwapMatcher;

impl CompositeMatcher for DedustSwapMatcher {
    fn try_match(&self, graph: &BaseActionGraph<'_>) -> Vec<CompositeMatch> {
        // A DeDust leg becomes a user-facing swap when it is paired with the payout
        // action from the Pool/Vault branch. The output may be a jetton wallet
        // transfer, a pTON transfer for wrapped TON routing, or a native TON payout.
        graph
            .base_actions()
            .iter()
            .filter(|action| {
                matches!(
                    action.kind,
                    BaseActionKind::DedustNativeSwapLeg | BaseActionKind::DedustJettonSwapLeg
                )
            })
            .filter_map(|swap_leg| {
                let payout_action = graph.children_of(swap_leg.id).find(|action| {
                    matches!(
                        action.kind,
                        BaseActionKind::JettonTransfer
                            | BaseActionKind::PtonTransfer
                            | BaseActionKind::DedustPayout
                    )
                })?;

                let offer_action = (swap_leg.kind == BaseActionKind::DedustJettonSwapLeg)
                    .then(|| direct_jetton_offer(graph, swap_leg))
                    .flatten();

                let mut nodes = BTreeSet::new();
                let mut base_actions = Vec::new();

                if let Some(offer_action) = offer_action {
                    nodes.extend(offer_action.nodes.iter().copied());
                    base_actions.push(offer_action.id);
                }

                nodes.extend(swap_leg.nodes.iter().copied());
                base_actions.push(swap_leg.id);

                nodes.extend(payout_action.nodes.iter().copied());
                base_actions.push(payout_action.id);

                Some(CompositeMatch {
                    kind: ActionKind::DedustSwap,
                    base_actions,
                    nodes,
                })
            })
            .collect()
    }
}

fn direct_jetton_offer<'a>(
    graph: &'a BaseActionGraph<'_>,
    swap_leg: &BaseAction,
) -> Option<&'a BaseAction> {
    graph.base_actions().iter().find(|action| {
        action.kind == BaseActionKind::JettonTransfer
            && graph
                .children_of(action.id)
                .any(|child| child.id == swap_leg.id)
    })
}

#[derive(Debug, Clone)]
struct DedustSwapInfo {
    offer: Option<AssetAmount>,
    ask: Option<AssetAmount>,
}

impl ActionInfo for DedustSwapInfo {
    fn render(&self) -> String {
        match (&self.offer, &self.ask) {
            (Some(offer), Some(ask)) => format!(
                "swapped {} to {} via DeDust",
                format_asset_amount(offer),
                format_asset_amount(ask),
            ),
            (Some(offer), None) => {
                format!("swapped {} via DeDust", format_asset_amount(offer))
            }
            (None, Some(ask)) => {
                format!("received {} via DeDust", format_asset_amount(ask))
            }
            (None, None) => "DeDust swap".to_owned(),
        }
    }
}

#[derive(Clone)]
struct DedustPayoutInfo {
    amount: Option<u128>,
    destination: Option<IntAddr>,
}

impl std::fmt::Debug for DedustPayoutInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DedustPayoutInfo")
            .field("amount", &self.amount)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for DedustPayoutInfo {
    fn render(&self) -> String {
        self.amount.map_or_else(
            || "received DeDust payout".to_owned(),
            |amount| format!("received DeDust payout of {}", format_ton_amount(amount)),
        )
    }
}

fn offer_amount(action: &Action, ctx: &EnrichmentContext<'_>) -> Option<AssetAmount> {
    native_offer(action, ctx).or_else(|| jetton_offer(action, ctx))
}

fn native_offer(action: &Action, ctx: &EnrichmentContext<'_>) -> Option<AssetAmount> {
    // For TON -> X the official Native Vault `swap#ea06185d` body carries the TON
    // `amount`, so no Pool event parsing is needed to recover the offered side.
    // Message value is a fallback for traces where decoded bodies are unavailable.
    let base_action = ctx.find_action_base(action, |base_action| {
        base_action.kind == BaseActionKind::DedustNativeSwapLeg
    })?;
    let root = ctx.fact_for_base(base_action)?;
    let amount = root
        .coins("amount")
        .or_else(|| root.message.as_ref().map(|msg| msg.value))?;

    Some(AssetAmount {
        asset: Asset::Ton,
        amount,
    })
}

fn jetton_offer(action: &Action, ctx: &EnrichmentContext<'_>) -> Option<AssetAmount> {
    let swap_leg_index = action.base_actions.iter().position(|id| {
        matches!(
            ctx.base_action(*id).map(|base_action| base_action.kind),
            Some(BaseActionKind::DedustJettonSwapLeg)
        )
    })?;
    let base_action = action.base_actions[..swap_leg_index]
        .iter()
        .filter_map(|id| ctx.base_action(*id))
        .find(|base_action| base_action.kind == BaseActionKind::JettonTransfer)?;

    jetton_transfer_amount(base_action, ctx)
}

fn payout_amount(action: &Action, ctx: &EnrichmentContext<'_>) -> Option<AssetAmount> {
    // The asked side is derived from the consumed payout action until we parse the
    // Pool `swap#9c610de3` event directly. For jetton-funded swaps the offer
    // transfer is placed before the swap leg, so payout lookup starts after it.
    let base_action = payout_base_action(action, ctx)?;

    match base_action.kind {
        BaseActionKind::JettonTransfer => jetton_transfer_amount(base_action, ctx),
        BaseActionKind::PtonTransfer => {
            let root = ctx.fact_for_base(base_action)?;
            let transfer = JettonTransferView::parse(root)?;
            Some(AssetAmount {
                asset: Asset::Ton,
                amount: transfer.amount()?,
            })
        }
        BaseActionKind::DedustPayout => {
            let root = ctx.fact_for_base(base_action)?;
            Some(AssetAmount {
                asset: Asset::Ton,
                amount: root.message.as_ref()?.value,
            })
        }
        _ => None,
    }
}

fn payout_base_action<'a>(action: &Action, ctx: &EnrichmentContext<'a>) -> Option<&'a BaseAction> {
    let start = swap_leg_index(action, ctx).map_or(0, |index| index + 1);

    action.base_actions[start..]
        .iter()
        .filter_map(|id| ctx.base_action(*id))
        .find(|base_action| {
            matches!(
                base_action.kind,
                BaseActionKind::JettonTransfer
                    | BaseActionKind::PtonTransfer
                    | BaseActionKind::DedustPayout
            )
        })
        .or_else(|| {
            ctx.find_action_base(action, |base_action| {
                matches!(
                    base_action.kind,
                    BaseActionKind::JettonTransfer
                        | BaseActionKind::PtonTransfer
                        | BaseActionKind::DedustPayout
                )
            })
        })
}

fn swap_leg_index(action: &Action, ctx: &EnrichmentContext<'_>) -> Option<usize> {
    action.base_actions.iter().position(|id| {
        matches!(
            ctx.base_action(*id).map(|base_action| base_action.kind),
            Some(BaseActionKind::DedustNativeSwapLeg) | Some(BaseActionKind::DedustJettonSwapLeg)
        )
    })
}

fn jetton_transfer_amount(
    base_action: &BaseAction,
    ctx: &EnrichmentContext<'_>,
) -> Option<AssetAmount> {
    let root = ctx.fact_for_base(base_action)?;
    let transfer = JettonTransferView::parse(root)?;

    Some(AssetAmount {
        asset: Asset::Jetton {
            wallet: root
                .message
                .as_ref()
                .and_then(|msg| msg.destination.clone()),
        },
        amount: transfer.amount()?,
    })
}
