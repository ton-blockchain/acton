import { useEffect, useMemo, useState } from 'react';
import { useTonAddress, useTonConnectUI } from '@tonconnect/ui-react';

import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Separator } from '@/components/ui/separator';
import {
  Avatar,
  Field,
  KV,
  Segmented,
  Select,
  hashHue,
  placeholderUrl,
  shortAddr,
} from './Primitives';
import { CollectionAddressInput } from './CollectionAddressInput';
import type { StoredCollection } from '../lib/collections';
import { DEFAULTS } from '../lib/nft/defaults';
import { buildBatchDeployTx, type BatchItem } from '../lib/nft/deploy';
import { getErrorMessage } from '../lib/nft/errors';
import type { Network } from '../lib/router';

type ContentMode = 'per-item' | 'shared';

export function DeployBatch({
  network,
  collections,
  selectedCollectionId,
  onSelectCollection,
  userWallet,
  onMinted,
}: {
  network: Network;
  collections: StoredCollection[];
  selectedCollectionId: string | null;
  onSelectCollection: (id: string) => void;
  userWallet: string;
  onMinted: (collectionId: string, count: number) => void;
}) {
  const walletAddress = useTonAddress();
  const [tonConnectUI] = useTonConnectUI();

  const collection =
    collections.find((c) => c.id === selectedCollectionId) ?? collections[0];
  const nextIndex = collection?.nextItemIndex ?? 0;

  const [owner, setOwner] = useState(userWallet || '');
  const [count, setCount] = useState('10');
  const [startIdxStr, setStartIdxStr] = useState(String(nextIndex));
  const [contentMode, setContentMode] = useState<ContentMode>('per-item');
  const [prefix, setPrefix] = useState('batch-');
  const [extension, setExtension] = useState('.json');
  const [sharedContent, setSharedContent] = useState('shared.json');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setStartIdxStr(String(nextIndex));
  }, [selectedCollectionId, nextIndex]);

  useEffect(() => {
    if (!owner && userWallet) setOwner(userWallet);
  }, [userWallet, owner]);

  const startIdx = parseInt(startIdxStr, 10);
  const startIdxBlank = !isNaN(startIdx);
  const startIdxBelow = startIdxBlank && startIdx < nextIndex;
  const startIdxValid = startIdxBlank && startIdx >= nextIndex;
  const startIdxSafe = startIdxValid ? startIdx : nextIndex;
  const skipsAhead = startIdxSafe > nextIndex;

  const rawN = parseInt(count, 10) || 0;
  const n = Math.max(0, Math.min(250, rawN));
  const overLimit = rawN > 250;

  const items = useMemo(
    () =>
      Array.from({ length: n }, (_, i) => ({
        index: startIdxSafe + i,
        content:
          contentMode === 'shared'
            ? sharedContent
            : `${prefix}${startIdxSafe + i}${extension}`,
      })),
    [n, startIdxSafe, prefix, extension, contentMode, sharedContent],
  );

  const perItemTon = parseFloat(DEFAULTS.perItemTonAmount);
  const batchTon = parseFloat(DEFAULTS.batchExtraValue);
  const totalGas = (perItemTon * n + batchTon).toFixed(3);

  const commonContent = collection?.commonContent || 'https://example.com/nft/';
  const sampleIdx = startIdxSafe;
  const sampleContent =
    contentMode === 'shared'
      ? sharedContent
      : `${prefix}${sampleIdx}${extension}`;

  const canDeploy =
    !!walletAddress &&
    !!collection &&
    n > 0 &&
    !overLimit &&
    startIdxValid &&
    !!owner.trim();

  async function handleDeploy() {
    if (!canDeploy || !collection) return;
    setError(null);
    setBusy(true);
    try {
      const batch: BatchItem[] = items.map((it) => ({
        index: BigInt(it.index),
        owner: owner.trim(),
        content: it.content,
      }));
      const request = buildBatchDeployTx(network, collection.address, batch);
      await tonConnectUI.sendTransaction(request);
      onMinted(collection.id, n);
      setStartIdxStr(String(startIdxSafe + n));
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1.05fr_1fr] gap-5 items-start">
      <Card>
        <CardHeader>
          <CardTitle>Deploy Batch</CardTitle>
          <CardDescription>
            Mints many items in a single transaction · up to 250 per batch
          </CardDescription>
        </CardHeader>

        <CardContent className="flex flex-col gap-3.5">
          <Field label="Collection">
            <Select<string>
              value={collection?.id}
              onChange={onSelectCollection}
              options={collections.map((c) => ({
                value: c.id,
                label: `${c.name} — ${shortAddr(c.address)}`,
              }))}
              placeholder={
                collections.length === 0
                  ? 'No collections yet — import by address or deploy one'
                  : 'Select a collection…'
              }
            />
          </Field>

          <CollectionAddressInput
            network={network}
            label="Or import collection by address"
            hint="Use an existing collection deployed elsewhere"
            onImported={(c) => onSelectCollection(c.id)}
          />

          <div className="grid grid-cols-2 gap-3.5">
            <Field
              label="Batch size"
              hint="Max 250 per tx"
              error={overLimit ? 'Max 250' : null}
            >
              <Input
                value={count}
                onChange={(e) =>
                  setCount(e.target.value.replace(/[^0-9]/g, ''))
                }
                inputMode="numeric"
              />
            </Field>
            <Field
              label="Starting index"
              hint={
                skipsAhead ? (
                  <>
                    Skipping ahead · next is{' '}
                    <span className="font-mono">#{nextIndex}</span>
                  </>
                ) : (
                  <>Next unminted index from collection</>
                )
              }
              error={
                !startIdxBlank ? (
                  'Enter a valid index'
                ) : startIdxBelow ? (
                  <>
                    Can't re-deploy existing items · minimum is{' '}
                    <span className="font-mono">#{nextIndex}</span>
                  </>
                ) : null
              }
            >
              <Input
                className="font-mono text-xs"
                value={startIdxStr}
                onChange={(e) =>
                  setStartIdxStr(e.target.value.replace(/[^0-9]/g, ''))
                }
                inputMode="numeric"
              />
            </Field>
          </div>

          <Field
            label="Owner for all items"
            hint={
              owner === userWallet
                ? 'Your connected wallet (pre-filled)'
                : 'Same owner for every item in the batch'
            }
          >
            <Input
              className="font-mono text-xs"
              value={owner}
              onChange={(e) => setOwner(e.target.value)}
              placeholder="EQ... / kQ..."
            />
          </Field>

          <Separator />

          <div className="flex items-center gap-2.5">
            <span className="text-xs font-semibold">Metadata per item</span>
            <span className="flex-1" />
            <Segmented<ContentMode>
              value={contentMode}
              onChange={setContentMode}
              options={[
                { value: 'per-item', label: 'Unique per item' },
                { value: 'shared', label: 'Same for all' },
              ]}
            />
          </div>

          <div className="uri-explainer">
            <div className="uri-explainer-label">Final URI =</div>
            <div className="uri-parts">
              <span
                className="uri-part uri-common"
                title="Collection's commonContent"
              >
                {commonContent}
              </span>
              <span className="uri-plus">+</span>
              {contentMode === 'shared' ? (
                <span
                  className="uri-part uri-item"
                  title="Shared content path — same for every item"
                >
                  {sampleContent || '…'}
                </span>
              ) : (
                <>
                  <span className="uri-part uri-prefix" title="Prefix">
                    {prefix}
                  </span>
                  <span className="uri-part uri-idx" title="Per-item index">
                    {sampleIdx}
                  </span>
                  <span className="uri-part uri-ext" title="Extension">
                    {extension}
                  </span>
                </>
              )}
            </div>
            <div className="uri-hint">
              The collection's <span className="font-mono">commonContent</span>{' '}
              is prepended on-chain to whatever you set here. Example shown for
              item <span className="font-mono">#{sampleIdx}</span>.
            </div>
          </div>

          {contentMode === 'per-item' ? (
            <div className="grid grid-cols-2 gap-3.5">
              <Field label="Content prefix">
                <Input
                  className="font-mono text-xs"
                  value={prefix}
                  onChange={(e) => setPrefix(e.target.value)}
                  placeholder="batch-"
                />
              </Field>
              <Field label="Extension">
                <Input
                  className="font-mono text-xs"
                  value={extension}
                  onChange={(e) => setExtension(e.target.value)}
                  placeholder=".json"
                />
              </Field>
            </div>
          ) : (
            <Field
              label="Shared content path"
              hint="Every item in the batch will point to the same metadata file."
            >
              <Input
                className="font-mono text-xs"
                value={sharedContent}
                onChange={(e) => setSharedContent(e.target.value)}
                placeholder="shared.json"
              />
            </Field>
          )}

          {error ? (
            <Alert variant="warning">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <Button
            size="lg"
            className="w-full"
            onClick={handleDeploy}
            disabled={!canDeploy || busy}
          >
            {busy
              ? 'Awaiting wallet…'
              : `Deploy ${n || 0} Item${n === 1 ? '' : 's'} · ${totalGas} GRAM`}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <div className="flex items-center gap-3.5 mb-4.5">
            <Avatar
              label={String(n)}
              tone={hashHue((collection?.name || '') + 'batch')}
              size={56}
            />
            <div>
              <div className="text-[15px] font-semibold tracking-tight">
                {n} item{n === 1 ? '' : 's'} · indices {startIdxSafe}–
                {startIdxSafe + Math.max(0, n - 1)}
              </div>
              <div className="text-[12.5px] text-primary font-mono mt-0.5">
                {collection?.name || 'No collection'}
              </div>
            </div>
          </div>

          <div className="flex flex-col">
            <KV
              k="Collection"
              v={collection ? shortAddr(collection.address) : '—'}
            />
            <KV k="Starting Index" v={startIdxSafe} />
            <KV k="Ending Index" v={n > 0 ? startIdxSafe + n - 1 : '—'} />
            <KV
              k="Metadata"
              v={contentMode === 'shared' ? 'Shared URI' : 'Unique per item'}
              mono={false}
            />
            <KV k="Per-Item Fee" v="0.03 GRAM" mono={false} />
            <KV k="Batch Fee" v="0.15 GRAM" mono={false} />
            <KV k="Total Cost" v={`${totalGas} GRAM`} />
          </div>

          <div className="mt-4">
            <div className="text-[10.5px] tracking-widest uppercase text-muted-foreground font-semibold mb-1.5">
              Item preview · {n}
            </div>
            <div className="batch-grid">
              {items.slice(0, 60).map((it) => (
                <div key={it.index} className="batch-thumb" title={it.content}>
                  <img
                    src={placeholderUrl(
                      (collection?.name || '') +
                        (contentMode === 'shared' ? 'shared' : it.index),
                      `#${it.index}`,
                    )}
                    alt=""
                  />
                  <div className="num">#{it.index}</div>
                </div>
              ))}
              {items.length > 60 ? (
                <div className="batch-thumb flex items-center justify-center text-muted-foreground text-[11px] font-mono">
                  +{items.length - 60}
                </div>
              ) : null}
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
