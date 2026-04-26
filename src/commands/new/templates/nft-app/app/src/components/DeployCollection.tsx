import { useState } from 'react';
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
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Separator } from '@/components/ui/separator';
import {
  Avatar,
  Field,
  KV,
  hashHue,
  placeholderUrl,
  shortAddr,
} from './Primitives';
import { buildDeployCollectionTx } from '../lib/nft/deploy';
import { getErrorMessage } from '../lib/nft/errors';
import { useCollectionsStore } from '../lib/collections';
import type { Network } from '../lib/router';
import { networkLabel } from '../lib/ton';

export function DeployCollection({
  network,
  userWallet,
  onDeployed,
}: {
  network: Network;
  userWallet: string;
  onDeployed: (address: string) => void;
}) {
  const walletAddress = useTonAddress();
  const [tonConnectUI] = useTonConnectUI();
  const { upsert } = useCollectionsStore(network);

  const [name, setName] = useState('TON Guardians');
  const [symbol, setSymbol] = useState('GUARD');
  const [description, setDescription] = useState(
    'A generative collection of on-chain guardians.',
  );
  const [image, setImage] = useState('');
  const [commonContent, setCommonContent] = useState(
    'https://example.com/nft/',
  );
  const [admin, setAdmin] = useState(userWallet || '');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const previewImage =
    image.trim() || placeholderUrl(name || 'TON', 'collection');
  const canDeploy =
    !!walletAddress &&
    !!admin.trim() &&
    !!name.trim() &&
    !!commonContent.trim();

  async function handleDeploy() {
    if (!walletAddress) {
      setError('Connect a wallet first.');
      return;
    }
    setError(null);
    setBusy(true);
    try {
      const built = buildDeployCollectionTx({
        network,
        admin: admin.trim(),
        metadata: {
          kind: 'onchain',
          name: name.trim(),
          description: description.trim(),
          image: image.trim(),
        },
        commonContent: commonContent.trim(),
        royaltyAddress: admin.trim(),
      });
      await tonConnectUI.sendTransaction(built.request);
      upsert({
        id: built.address,
        address: built.address,
        name: name.trim() || 'Untitled collection',
        symbol: symbol.trim(),
        admin: admin.trim(),
        commonContent: commonContent.trim(),
        metadataUri: '',
        description: description.trim(),
        image: image.trim(),
        royaltyPercent: 5,
        nextItemIndex: 0,
        createdAt: Date.now(),
      });
      onDeployed(built.address);
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
          <CardTitle>Deploy New Collection</CardTitle>
          <CardDescription>
            Creates an NFT Collection contract on TON {networkLabel(network)} ·
            TEP-62 / TEP-64
          </CardDescription>
        </CardHeader>

        <CardContent className="flex flex-col gap-3.5">
          <div className="grid grid-cols-2 gap-3.5">
            <Field label="Collection name">
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. TON Guardians"
              />
            </Field>
            <Field label="Symbol" hint="Shown in wallets & marketplaces">
              <Input
                value={symbol}
                onChange={(e) => setSymbol(e.target.value.toUpperCase())}
                placeholder="GUARD"
                maxLength={10}
              />
            </Field>

            <div className="col-span-2">
              <Field label="Description">
                <Textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Brief description of the collection"
                  rows={3}
                />
              </Field>
            </div>

            <div className="col-span-2">
              <Field label="Image URL" hint="PNG/SVG recommended, 512x512px">
                <Input
                  value={image}
                  onChange={(e) => setImage(e.target.value)}
                  placeholder="https://example.com/cover.png"
                />
              </Field>
            </div>

            <div className="col-span-2">
              <Field
                label="Common content URI"
                hint="Base URI prepended to each item's content"
              >
                <Input
                  className="font-mono text-xs"
                  value={commonContent}
                  onChange={(e) => setCommonContent(e.target.value)}
                  placeholder="https://example.com/nft/"
                />
              </Field>
            </div>
          </div>

          <Field
            label="Collection admin"
            hint={
              admin === userWallet
                ? 'Your connected wallet (pre-filled) — can deploy items and change admin'
                : 'This address will control the collection'
            }
          >
            <Input
              className="font-mono text-xs"
              value={admin}
              onChange={(e) => setAdmin(e.target.value)}
              placeholder="EQ... / kQ..."
            />
          </Field>

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
            {busy ? 'Awaiting wallet…' : 'Deploy Collection'}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <div className="flex items-center gap-3.5 mb-4.5">
            <Avatar
              label={symbol || name}
              image={image.trim() ? image.trim() : null}
              size={56}
              tone={hashHue(name + symbol)}
            />
            <div>
              <div className="text-[15px] font-semibold tracking-tight">
                {name || 'Untitled Collection'}
              </div>
              <div className="text-[12.5px] text-primary font-mono mt-0.5">
                {symbol ? (
                  `#${symbol}`
                ) : (
                  <span className="text-muted-foreground">No symbol</span>
                )}
              </div>
            </div>
          </div>

          <div className="relative w-full rounded-lg overflow-hidden bg-secondary border border-border aspect-square flex items-center justify-center mb-4.5">
            <img
              src={previewImage}
              alt="collection preview"
              className="w-full h-full object-cover"
            />
          </div>

          <div className="flex flex-col">
            <KV k="Standard" v="TEP-62 NFT" mono={false} />
            <KV k="Next Index" v="0" />
            <KV
              k="Admin"
              v={
                admin ? (
                  shortAddr(admin)
                ) : (
                  <span className="text-muted-foreground">—</span>
                )
              }
            />
            <KV k="Common Content" v={commonContent || '—'} />
          </div>

          <Separator className="mt-2" />
          <div className="mt-2.5">
            <div className="text-[10.5px] tracking-widest uppercase text-muted-foreground font-semibold mb-1.5">
              About
            </div>
            <p className="m-0 text-[13px] text-muted-foreground">
              {description || (
                <span className="text-muted-foreground">
                  No description provided.
                </span>
              )}
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
