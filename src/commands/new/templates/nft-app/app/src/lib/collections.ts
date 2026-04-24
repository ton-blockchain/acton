import { useCallback, useSyncExternalStore } from 'react';

import type { Network } from './router';

export interface StoredCollection {
  id: string;
  address: string;
  name: string;
  symbol: string;
  admin: string;
  commonContent: string;
  metadataUri: string;
  description: string;
  image: string;
  royaltyPercent: number;
  nextItemIndex: number;
  createdAt: number;
}

function storageKey(network: Network) {
  return `nft-minter:collections:${network}`;
}

function readFromStorage(network: Network): StoredCollection[] {
  try {
    const raw = localStorage.getItem(storageKey(network));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed;
  } catch {
    return [];
  }
}

function writeToStorage(network: Network, list: StoredCollection[]) {
  localStorage.setItem(storageKey(network), JSON.stringify(list));
}

// Module-level singleton: one snapshot per network, one set of subscribers.
// All hook instances see the same data, so updating from one component
// (e.g. CollectionAddressInput) immediately notifies the App tree.
const cache = new Map<Network, StoredCollection[]>();
const subscribers = new Set<() => void>();

function getSnapshot(network: Network): StoredCollection[] {
  let snapshot = cache.get(network);
  if (!snapshot) {
    snapshot = readFromStorage(network);
    cache.set(network, snapshot);
  }
  return snapshot;
}

function setSnapshot(network: Network, next: StoredCollection[]) {
  cache.set(network, next);
  writeToStorage(network, next);
  subscribers.forEach((cb) => cb());
}

function subscribe(cb: () => void): () => void {
  subscribers.add(cb);
  return () => {
    subscribers.delete(cb);
  };
}

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (!e.key || !e.key.startsWith('nft-minter:collections:')) return;
    const network = e.key.split(':').pop() as Network;
    cache.set(network, readFromStorage(network));
    subscribers.forEach((cb) => cb());
  });
}

export function useCollectionsStore(network: Network) {
  const list = useSyncExternalStore(
    subscribe,
    () => getSnapshot(network),
    () => getSnapshot(network),
  );

  const upsert = useCallback(
    (entry: StoredCollection) => {
      const prev = getSnapshot(network);
      const existing = prev.findIndex((c) => c.address === entry.address);
      let next: StoredCollection[];
      if (existing >= 0) {
        next = prev.slice();
        next[existing] = { ...prev[existing], ...entry };
      } else {
        next = [entry, ...prev];
      }
      setSnapshot(network, next);
    },
    [network],
  );

  const update = useCallback(
    (id: string, patch: Partial<StoredCollection>) => {
      const prev = getSnapshot(network);
      const next = prev.map((c) => (c.id === id ? { ...c, ...patch } : c));
      setSnapshot(network, next);
    },
    [network],
  );

  const remove = useCallback(
    (id: string) => {
      const prev = getSnapshot(network);
      const next = prev.filter((c) => c.id !== id);
      setSnapshot(network, next);
    },
    [network],
  );

  return { list, upsert, update, remove };
}
