use crate::storage::JettonMasterMeta;
use crate::types::Addr;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinSet;
use ton_api::OffchainJsonResolver;
use ton_indexer_contracts::jettons;

pub(crate) async fn enrich_jetton_masters(
    resolver: &OffchainJsonResolver,
    masters: &mut [JettonMasterMeta],
) {
    let remote_by_uri = resolve_jetton_metadata(
        resolver,
        masters
            .iter()
            .filter_map(|master| jetton_content_uri(&master.jetton_content)),
    )
    .await;
    for master in masters {
        merge_resolved_metadata(&mut master.jetton_content, &remote_by_uri);
    }
}

pub(crate) async fn enrich_jetton_master_map(
    resolver: &OffchainJsonResolver,
    masters: &mut HashMap<Addr, JettonMasterMeta>,
) {
    let remote_by_uri = resolve_jetton_metadata(
        resolver,
        masters
            .values()
            .filter_map(|master| jetton_content_uri(&master.jetton_content)),
    )
    .await;
    for master in masters.values_mut() {
        merge_resolved_metadata(&mut master.jetton_content, &remote_by_uri);
    }
}

pub(crate) fn jetton_content_uri(content: &serde_json::Value) -> Option<String> {
    jettons::jetton_content_uri(content).map(str::to_owned)
}

pub(crate) async fn resolve_jetton_metadata(
    resolver: &OffchainJsonResolver,
    uris: impl Iterator<Item = String>,
) -> HashMap<String, Arc<serde_json::Value>> {
    let mut pending = JoinSet::new();
    let mut seen = HashSet::new();
    for uri in uris {
        if !seen.insert(uri.clone()) {
            continue;
        }
        let resolver = resolver.clone();
        pending.spawn(async move {
            let result = resolver.get_json(&uri).await;
            (uri, result)
        });
    }

    let mut resolved = HashMap::new();
    while let Some(result) = pending.join_next().await {
        match result {
            Ok((uri, Ok(metadata))) => {
                resolved.insert(uri, metadata);
            }
            Ok((uri, Err(error))) => {
                tracing::debug!(%uri, %error, "Failed to load off-chain jetton metadata");
            }
            Err(error) => {
                tracing::debug!(%error, "Off-chain metadata task failed");
            }
        }
    }
    resolved
}

pub(crate) fn merge_resolved_metadata(
    content: &mut serde_json::Value,
    remote_by_uri: &HashMap<String, Arc<serde_json::Value>>,
) {
    let Some(uri) = jettons::jetton_content_uri(content) else {
        return;
    };
    let Some(remote) = remote_by_uri.get(uri) else {
        return;
    };
    jettons::merge_jetton_content(content, remote);
}
