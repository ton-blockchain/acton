import { useState } from 'react';
import { useTonConnectUI } from '@tonconnect/ui-react';
import { Copy, ExternalLink, RefreshCw, Send, User } from 'lucide-react';

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
import { Avatar, Field, KV, Select, hashHue, shortAddr } from './Primitives';
import { CollectionAddressInput } from './CollectionAddressInput';
import type { StoredCollection } from '../lib/collections';
import { ZERO_ADDRESS } from '../lib/nft/address';
import { getErrorMessage } from '../lib/nft/errors';
import { getNftAddressByIndex, readCollection } from '../lib/nft/read';
import { buildChangeAdminTx, buildTransferItemTx } from '../lib/nft/transfer';
import type { Network } from '../lib/router';
import { tonviewerUrl } from '../lib/ton';

export function Manage({
  network,
  collections,
  selectedCollectionId,
  onSelectCollection,
  onRefreshCollection,
}: {
  network: Network;
  collections: StoredCollection[];
  selectedCollectionId: string | null;
  onSelectCollection: (id: string) => void;
  onRefreshCollection: (id: string, patch: Partial<StoredCollection>) => void;
}) {
  const [tonConnectUI] = useTonConnectUI();

  const [newAdmin, setNewAdmin] = useState('');
  const [transferItemIdx, setTransferItemIdx] = useState('');
  const [transferNewOwner, setTransferNewOwner] = useState('');
  const [royaltyStatus, setRoyaltyStatus] = useState<string | null>(null);
  const [royaltyLoading, setRoyaltyLoading] = useState(false);
  const [adminError, setAdminError] = useState<string | null>(null);
  const [transferError, setTransferError] = useState<string | null>(null);
  const [adminBusy, setAdminBusy] = useState(false);
  const [transferBusy, setTransferBusy] = useState(false);

  const collection =
    collections.find((c) => c.id === selectedCollectionId) ?? collections[0];

  if (!collection) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>No collections yet</CardTitle>
          <CardDescription>
            Deploy one in the Create tab, or load an existing collection by
            address below.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3.5">
          <CollectionAddressInput
            network={network}
            onImported={(c) => onSelectCollection(c.id)}
          />
        </CardContent>
      </Card>
    );
  }

  const transferIdxNum = parseInt(transferItemIdx, 10);
  const transferIdxEntered =
    transferItemIdx !== '' && !isNaN(transferIdxNum) && transferIdxNum >= 0;

  const royaltyPct = collection.royaltyPercent;

  async function onCopyAddress() {
    try {
      await navigator.clipboard.writeText(collection!.address);
    } catch {
      /* noop */
    }
  }

  async function onRequestRoyalty() {
    if (!collection) return;
    setRoyaltyStatus(null);
    setRoyaltyLoading(true);
    try {
      const onchain = await readCollection(network, collection.address);
      if (onchain) {
        onRefreshCollection(collection.id, {
          nextItemIndex: onchain.nextItemIndex,
          admin: onchain.adminAddress,
          royaltyPercent:
            onchain.royaltyDenominator > 0
              ? Math.round(
                  (onchain.royaltyNumerator * 100) / onchain.royaltyDenominator,
                )
              : 0,
        });
        setRoyaltyStatus(
          `Fetched from chain · next index #${onchain.nextItemIndex}`,
        );
      } else {
        setRoyaltyStatus('Collection is not deployed on-chain yet.');
      }
    } catch (err) {
      setRoyaltyStatus(getErrorMessage(err));
    } finally {
      setRoyaltyLoading(false);
    }
  }

  async function onChangeAdmin() {
    if (!collection || !newAdmin.trim()) return;
    setAdminError(null);
    setAdminBusy(true);
    try {
      const request = buildChangeAdminTx(
        network,
        collection.address,
        newAdmin.trim(),
      );
      await tonConnectUI.sendTransaction(request);
      onRefreshCollection(collection.id, { admin: newAdmin.trim() });
      setNewAdmin('');
    } catch (err) {
      setAdminError(getErrorMessage(err));
    } finally {
      setAdminBusy(false);
    }
  }

  function onRevokeAdmin() {
    setNewAdmin(ZERO_ADDRESS);
  }

  async function onTransferItem() {
    if (!collection || !transferNewOwner.trim() || !transferIdxEntered) return;
    setTransferError(null);
    setTransferBusy(true);
    try {
      const onchain = await readCollection(network, collection.address);
      if (!onchain) {
        setTransferError('Collection not found on-chain.');
        return;
      }
      if (transferIdxNum >= onchain.nextItemIndex) {
        setTransferError(
          onchain.nextItemIndex === 0
            ? 'Collection has no minted items yet.'
            : `Out of range · max minted index is #${onchain.nextItemIndex - 1}.`,
        );
        onRefreshCollection(collection.id, {
          nextItemIndex: onchain.nextItemIndex,
        });
        return;
      }
      onRefreshCollection(collection.id, {
        nextItemIndex: onchain.nextItemIndex,
      });
      const itemAddr = await getNftAddressByIndex(
        network,
        collection.address,
        BigInt(transferIdxNum),
      );
      const request = buildTransferItemTx(
        network,
        itemAddr,
        transferNewOwner.trim(),
      );
      await tonConnectUI.sendTransaction(request);
      setTransferNewOwner('');
    } catch (err) {
      setTransferError(getErrorMessage(err));
    } finally {
      setTransferBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <Card>
        <CardContent>
          <div className="flex items-center gap-4 mb-5 flex-wrap">
            <Avatar
              label={collection.symbol || collection.name}
              size={64}
              tone={hashHue(collection.name)}
              image={collection.image || null}
            />
            <div className="min-w-0 flex-1">
              <div className="text-xl font-semibold tracking-tight">
                {collection.name}
              </div>
              <div className="flex items-center gap-1.5 mt-1">
                <span className="font-mono text-xs text-muted-foreground">
                  {shortAddr(collection.address, 10, 10)}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6 rounded-md"
                  title="Copy address"
                  onClick={onCopyAddress}
                >
                  <Copy className="size-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6 rounded-md"
                  asChild
                >
                  <a
                    title="Open in explorer"
                    href={`${tonviewerUrl(network)}/${encodeURIComponent(collection.address)}`}
                    target="_blank"
                    rel="noreferrer"
                  >
                    <ExternalLink className="size-3" />
                  </a>
                </Button>
              </div>
            </div>
            <div className="min-w-[260px] max-md:flex-grow max-md:min-w-0">
              <Select<string>
                value={collection.id}
                onChange={onSelectCollection}
                options={collections.map((c) => ({
                  value: c.id,
                  label: `${c.name} — ${shortAddr(c.address)}`,
                }))}
                placeholder="Switch collection…"
              />
            </div>
          </div>

          <div className="mb-4">
            <CollectionAddressInput
              network={network}
              onImported={(c) => onSelectCollection(c.id)}
            />
          </div>

          <div className="info-row">
            <div className="info-cell">
              <div className="info-key">Items minted</div>
              <div className="info-val font-mono">
                {collection.nextItemIndex}
              </div>
            </div>
            <div className="info-cell">
              <div className="info-key">Next Index</div>
              <div className="info-val font-mono">
                #{collection.nextItemIndex}
              </div>
            </div>
            <div className="info-cell">
              <div className="info-key">Royalty</div>
              <div className="info-val font-mono">{royaltyPct}%</div>
            </div>
            <div className="info-cell">
              <div className="info-key">Standard</div>
              <div className="info-val">TEP-62</div>
            </div>
            <div className="info-cell wide">
              <div className="info-key">Admin</div>
              <div className="info-val font-mono truncate">
                {collection.admin}
              </div>
            </div>
            <div className="info-cell wide">
              <div className="info-key">Common Content</div>
              <div className="info-val font-mono truncate">
                {collection.commonContent}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-5">
        <Card>
          <CardHeader>
            <div className="flex items-center gap-2">
              <User className="size-4 text-muted-foreground" />
              <CardTitle>Change admin</CardTitle>
            </div>
            <CardDescription>
              Transfers admin rights. The new admin will be able to deploy items
              and change admin.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3.5">
            <Field label="New admin">
              <Input
                className="font-mono text-xs"
                value={newAdmin}
                onChange={(e) => setNewAdmin(e.target.value)}
                placeholder="EQ... / kQ..."
              />
            </Field>
            {adminError ? (
              <Alert variant="warning">
                <AlertDescription>{adminError}</AlertDescription>
              </Alert>
            ) : null}
            <Button
              variant="secondary"
              disabled={!newAdmin.trim() || adminBusy}
              onClick={onChangeAdmin}
            >
              Transfer admin
            </Button>
            <Separator />
            <Button variant="destructive" onClick={onRevokeAdmin}>
              Revoke admin
            </Button>
            <div className="text-[11.5px] text-muted-foreground -mt-1">
              Transfers admin rights to{' '}
              <span className="font-mono">addr_none</span> · this action is
              irreversible.
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div className="flex items-center gap-2">
              <Send className="size-4 text-muted-foreground" />
              <CardTitle>Transfer item</CardTitle>
            </div>
            <CardDescription>
              Sends <span className="font-mono">AskToChangeOwnership</span> to
              the specified item.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3.5">
            <Field label="Item #">
              <Input
                className="font-mono text-xs"
                value={transferItemIdx}
                onChange={(e) => {
                  setTransferItemIdx(e.target.value.replace(/[^0-9]/g, ''));
                  setTransferError(null);
                }}
                placeholder="0"
              />
            </Field>
            <Field label="New owner">
              <Input
                className="font-mono text-xs"
                value={transferNewOwner}
                onChange={(e) => setTransferNewOwner(e.target.value)}
                placeholder="EQ... / kQ..."
              />
            </Field>
            {transferError ? (
              <Alert variant="warning">
                <AlertDescription>{transferError}</AlertDescription>
              </Alert>
            ) : null}
            <Button
              variant="secondary"
              disabled={
                !transferNewOwner.trim() || !transferIdxEntered || transferBusy
              }
              onClick={onTransferItem}
            >
              {transferBusy ? 'Checking…' : 'Send transfer'}
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div className="flex items-center gap-2">
              <RefreshCw className="size-4 text-muted-foreground" />
              <CardTitle>Royalty params</CardTitle>
            </div>
            <CardDescription>
              Re-reads <span className="font-mono">royalty_params</span> and{' '}
              <span className="font-mono">get_collection_data</span> from chain.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3.5">
            <div className="flex flex-col">
              <KV k="Percent" v={`${royaltyPct}%`} />
              <KV k="Numerator" v={Math.round(royaltyPct * 10)} />
              <KV k="Denominator" v="1000" />
              <KV k="Recipient" v={shortAddr(collection.admin)} />
            </div>
            {royaltyStatus ? (
              <div className="text-[11.5px] text-muted-foreground">
                {royaltyStatus}
              </div>
            ) : null}
            <Button
              variant="secondary"
              onClick={onRequestRoyalty}
              disabled={royaltyLoading}
            >
              {royaltyLoading ? 'Reading…' : 'Request from chain'}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
