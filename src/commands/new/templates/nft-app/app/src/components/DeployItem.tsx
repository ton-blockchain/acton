import { useEffect, useState } from 'react';
import { useTonAddress, useTonConnectUI } from '@tonconnect/ui-react';
import { AlertTriangle } from 'lucide-react';

import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Separator } from '@/components/ui/separator';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import {
  Avatar,
  Field,
  KV,
  Select,
  hashHue,
  placeholderUrl,
  shortAddr,
} from './Primitives';
import { CollectionAddressInput } from './CollectionAddressInput';
import type { StoredCollection } from '../lib/collections';
import { buildDeployItemTx } from '../lib/nft/deploy';
import { getErrorMessage } from '../lib/nft/errors';
import type { Network } from '../lib/router';

export function DeployItem({
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
  onMinted: (collectionId: string) => void;
}) {
  const walletAddress = useTonAddress();
  const [tonConnectUI] = useTonConnectUI();

  const collection =
    collections.find((c) => c.id === selectedCollectionId) ?? collections[0];
  const nextIndex = collection?.nextItemIndex ?? 0;
  const lastIndex = Math.max(0, nextIndex - 1);

  const [owner, setOwner] = useState(userWallet || '');
  const [itemIndex, setItemIndex] = useState(String(nextIndex));
  const [content, setContent] = useState(`item-${nextIndex}.json`);
  const [advanced, setAdvanced] = useState(false);
  const [rawContent, setRawContent] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setItemIndex(String(nextIndex));
    setContent(`item-${nextIndex}.json`);
  }, [selectedCollectionId, nextIndex]);

  useEffect(() => {
    if (!owner && userWallet) setOwner(userWallet);
  }, [userWallet, owner]);

  const idxNum = parseInt(itemIndex, 10);
  const idxValid = !isNaN(idxNum) && idxNum >= 0;
  const isOverwrite = idxValid && idxNum < nextIndex;
  const isNext = idxValid && idxNum === nextIndex;
  const isSkip = idxValid && idxNum > nextIndex;

  const effectiveContent = advanced && rawContent.trim() ? rawContent : content;
  const fullUri = (collection?.commonContent || '') + effectiveContent;

  const canDeploy =
    !!walletAddress && !!collection && idxValid && !!owner.trim();

  async function handleDeploy() {
    if (!canDeploy || !collection) return;
    setError(null);
    setBusy(true);
    try {
      const request = buildDeployItemTx({
        network,
        collectionAddress: collection.address,
        itemIndex: BigInt(idxNum),
        itemOwner: owner.trim(),
        itemContent: effectiveContent,
      });
      await tonConnectUI.sendTransaction(request);
      onMinted(collection.id);
      const advancedIdx = idxNum + 1;
      setItemIndex(String(advancedIdx));
      setContent(`item-${advancedIdx}.json`);
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
          <CardTitle>Deploy NFT Item</CardTitle>
          <CardDescription>
            Mints a single item into an existing collection
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

          <Field
            label="Item index"
            hint={
              isNext ? (
                <>
                  Next unminted index · increments{' '}
                  <span className="font-mono">nextItemIndex</span>
                </>
              ) : isOverwrite ? (
                <>
                  ⚠ Will re-deploy an existing item. Last minted:{' '}
                  <span className="font-mono">#{lastIndex}</span>
                </>
              ) : isSkip ? (
                <>
                  Skipping ahead. Current collection's{' '}
                  <span className="font-mono">nextItemIndex</span> is{' '}
                  <span className="font-mono">{nextIndex}</span>
                </>
              ) : (
                <>Enter a 0-based index</>
              )
            }
            error={!idxValid ? 'Invalid index' : null}
          >
            <Input
              className="font-mono text-xs"
              value={itemIndex}
              onChange={(e) =>
                setItemIndex(e.target.value.replace(/[^0-9]/g, ''))
              }
              inputMode="numeric"
            />
          </Field>

          <Field
            label="Owner address"
            hint={
              owner === userWallet
                ? 'Your connected wallet (pre-filled)'
                : 'Item will be owned by this address'
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

          <Field
            label="Item content path"
            hint={
              <>
                Appended to{' '}
                <span className="font-mono">
                  {collection?.commonContent || 'commonContent'}
                </span>{' '}
                in the collection
              </>
            }
          >
            <Input
              className="font-mono text-xs"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="item-0.json"
            />
          </Field>

          <div className="flex items-center gap-2">
            <Switch
              id="advanced-toggle"
              checked={advanced}
              onCheckedChange={setAdvanced}
            />
            <Label
              htmlFor="advanced-toggle"
              className="text-xs text-muted-foreground cursor-pointer"
            >
              Override with raw metadata
            </Label>
          </div>

          {advanced ? (
            <Field
              label="Raw content (snake-encoded string)"
              hint="Overrides the path above. Use for custom per-item URIs or inline JSON."
            >
              <Textarea
                value={rawContent}
                onChange={(e) => setRawContent(e.target.value)}
                placeholder="ipfs://Qm..../42.json"
                rows={3}
              />
            </Field>
          ) : null}

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
            {busy ? (
              'Awaiting wallet…'
            ) : (
              <>
                Deploy Item{' '}
                <span className="font-mono opacity-70">
                  #{idxValid ? idxNum : '?'}
                </span>
              </>
            )}
          </Button>

          {isOverwrite ? (
            <Alert variant="warning">
              <AlertTriangle className="size-4" />
              <AlertDescription>
                Index <span className="font-mono">#{idxNum}</span> has already
                been minted. Re-deploying will not overwrite existing storage;
                the tx may bounce.
              </AlertDescription>
            </Alert>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <div className="flex items-center gap-3.5 mb-4.5">
            <Avatar
              label={String(idxValid ? idxNum : '?')}
              tone={hashHue((collection?.name || '') + (idxNum || 0))}
              size={56}
            />
            <div>
              <div className="text-[15px] font-semibold tracking-tight">
                {collection?.name || 'No collection'} #{idxValid ? idxNum : '?'}
              </div>
              <div className="text-[12.5px] text-primary font-mono mt-0.5">
                Item index {idxValid ? idxNum : '—'}
              </div>
            </div>
          </div>

          <div className="relative w-full rounded-lg overflow-hidden bg-secondary border border-border aspect-square flex items-center justify-center mb-4.5">
            <img
              src={placeholderUrl(
                (collection?.name || '') + (idxNum || 0),
                `#${idxValid ? idxNum : '?'}`,
              )}
              alt=""
              className="w-full h-full object-cover"
            />
            <div className="absolute bottom-2 right-2 px-2 py-0.5 bg-black/55 backdrop-blur-sm rounded-md font-mono text-[11px] text-white">
              #{idxValid ? idxNum : '?'}
            </div>
          </div>

          <div className="flex flex-col">
            <KV
              k="Collection"
              v={collection ? shortAddr(collection.address) : '—'}
            />
            <KV
              k="Owner"
              v={
                owner ? (
                  shortAddr(owner)
                ) : (
                  <span className="text-muted-foreground">—</span>
                )
              }
            />
            <KV k="Item Index" v={idxValid ? idxNum : '—'} />
            <KV k="Content" v={effectiveContent || '—'} />
            <KV k="Full URI" v={fullUri || '—'} />
            <KV k="Storage Fee" v="~0.03 GRAM" mono={false} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
