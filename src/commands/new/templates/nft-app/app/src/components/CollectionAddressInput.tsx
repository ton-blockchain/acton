import { useState } from 'react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Field } from './Primitives';
import { parseAddr } from '../lib/nft/address';
import { getErrorMessage } from '../lib/nft/errors';
import { readCollection } from '../lib/nft/read';
import { useCollectionsStore, type StoredCollection } from '../lib/collections';
import type { Network } from '../lib/router';

export function CollectionAddressInput({
  network,
  label = 'Import collection by address',
  hint = 'Load an existing collection deployed on-chain',
  onImported,
}: {
  network: Network;
  label?: string;
  hint?: string;
  onImported?: (collection: StoredCollection) => void;
}) {
  const { upsert, list } = useCollectionsStore(network);
  const [address, setAddress] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  async function handleLoad() {
    setError(null);
    setStatus(null);
    const trimmed = address.trim();
    if (!trimmed) return;
    try {
      parseAddr(trimmed);
    } catch {
      setError('Invalid TON address');
      return;
    }
    setBusy(true);
    try {
      const onchain = await readCollection(network, trimmed);
      if (!onchain) {
        setError(
          'Contract is not deployed or not a collection on this network.',
        );
        return;
      }
      const royaltyPercent =
        onchain.royaltyDenominator > 0
          ? Math.round(
              (onchain.royaltyNumerator * 100) / onchain.royaltyDenominator,
            )
          : 0;
      const existing = list.find((c) => c.address === onchain.address);
      const entry: StoredCollection = {
        id: onchain.address,
        address: onchain.address,
        name: onchain.metadata.name || existing?.name || 'Imported collection',
        symbol: existing?.symbol || '',
        admin: onchain.adminAddress,
        commonContent: onchain.commonContent || existing?.commonContent || '',
        metadataUri: onchain.metadata.uri || existing?.metadataUri || '',
        description:
          onchain.metadata.description || existing?.description || '',
        image: onchain.metadata.image || existing?.image || '',
        royaltyPercent,
        nextItemIndex: onchain.nextItemIndex,
        createdAt: existing?.createdAt ?? Date.now(),
      };
      upsert(entry);
      onImported?.(entry);
      setAddress('');
      setStatus(
        `Loaded · ${royaltyPercent}% royalty · next index #${onchain.nextItemIndex}`,
      );
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Field
      label={label}
      hint={error ? null : (status ?? hint)}
      error={error ?? undefined}
    >
      <div className="flex items-center gap-2">
        <div className="flex-1 min-w-0">
          <Input
            className="font-mono text-xs"
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            placeholder="EQ... / kQ..."
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleLoad();
            }}
          />
        </div>
        <Button
          variant="secondary"
          onClick={handleLoad}
          disabled={!address.trim() || busy}
        >
          {busy ? 'Loading…' : 'Load'}
        </Button>
      </div>
    </Field>
  );
}
