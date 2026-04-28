import { useState, useCallback, useEffect, type FormEvent } from 'react';
import { useTonConnectUI, useTonWallet } from '@tonconnect/ui-react';
import { Address, toNano } from '@ton/core';
import { Search, AlertCircle, Wallet, Lock, Check } from 'lucide-react';
import { getTonClient, getWalletAddress, fetchJettonMaster } from '../lib/ton';
import type { JettonMetadata } from '../lib/jettonContent';
import {
  buildMintBody,
  buildChangeAdminBody,
  buildChangeContentBody,
  buildBurnBody,
  buildTransferBody,
  parseUnits,
} from '../lib/deploy';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { Avatar, AvatarImage, AvatarFallback } from '@/components/ui/avatar';
import { StatusAlert, PreviewRow, NetworkBadge } from './DeployPage';
import { useTheme } from '../App';

const DEFAULT_JETTON_MAINNET =
  'EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs';
const DEFAULT_JETTON_TESTNET =
  'kQAzgQ4T081rhYewF9g19vJIX1iRCy_31OvgzFPtfEM3ivw0';
const ZERO_ADDRESS =
  '0:0000000000000000000000000000000000000000000000000000000000000000';

function tryParseAddress(raw: string): Address | null {
  try {
    return Address.parse(raw.trim());
  } catch {
    return null;
  }
}

interface Props {
  network: 'mainnet' | 'testnet';
  initialAddress: string | null;
  onAddressChange: (address: string) => void;
}

interface JettonInfo {
  totalSupply: bigint;
  mintable: boolean;
  adminAddress: Address | null;
  metadata: Partial<JettonMetadata>;
}

type ManageTab = 'mint' | 'transfer' | 'burn' | 'admin';

export function ManagePage({
  network,
  initialAddress,
  onAddressChange,
}: Props) {
  const [tonConnectUI] = useTonConnectUI();
  const wallet = useTonWallet();

  const defaultAddr =
    network === 'testnet' ? DEFAULT_JETTON_TESTNET : DEFAULT_JETTON_MAINNET;
  const [contractAddr, setContractAddrRaw] = useState(
    initialAddress || defaultAddr,
  );

  function setContractAddr(addr: string) {
    setContractAddrRaw(addr);
    onAddressChange(addr);
    setJettonInfo(null);
    setStatus(null);
  }
  const [jettonInfo, setJettonInfo] = useState<JettonInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<ManageTab>('mint');
  const [status, setStatus] = useState<{
    type: 'success' | 'error' | 'info';
    message: string;
  } | null>(null);

  const ownerAddress = wallet?.account?.address
    ? Address.parse(wallet.account.address)
    : null;

  const isConnected = !!wallet;
  const { theme } = useTheme();

  const loadJettonInfo = useCallback(async () => {
    if (!contractAddr.trim()) {
      setStatus({ type: 'error', message: 'Enter a contract address' });
      return;
    }

    setLoading(true);
    setStatus(null);
    setJettonInfo(null);

    try {
      const data = await fetchJettonMaster(network, contractAddr.trim());
      setJettonInfo({
        totalSupply: data.totalSupply,
        mintable: data.mintable,
        adminAddress: data.adminAddress,
        metadata: data.metadata,
      });
    } catch (err: any) {
      const msg = err?.message || '';
      if (
        msg.includes('exit_code') ||
        msg.includes('-13') ||
        msg.includes('unable to execute')
      ) {
        const otherNet = network === 'mainnet' ? 'Testnet' : 'Mainnet';
        setStatus({
          type: 'error',
          message: `Contract not found on ${network === 'mainnet' ? 'Mainnet' : 'Testnet'}. Make sure the address is correct or try switching to ${otherNet}.`,
        });
      } else {
        setStatus({
          type: 'error',
          message: msg || 'Failed to load jetton data',
        });
      }
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }, [contractAddr, network]);

  useEffect(() => {
    if (contractAddr.trim()) {
      onAddressChange(contractAddr.trim());
      loadJettonInfo();
    }
  }, [network]);

  const isAdmin =
    jettonInfo && ownerAddress && jettonInfo.adminAddress
      ? jettonInfo.adminAddress.equals(ownerAddress)
      : false;

  const decimals = parseInt(jettonInfo?.metadata?.decimals || '9') || 9;

  function formatAmount(amount: bigint): string {
    const divisor = 10n ** BigInt(decimals);
    const whole = amount / divisor;
    const remainder = amount % divisor;
    if (remainder === 0n) return whole.toString();
    const fracStr = remainder
      .toString()
      .padStart(decimals, '0')
      .replace(/0+$/, '');
    return `${whole}.${fracStr}`;
  }

  return (
    <div className="grid grid-cols-[1fr_320px] gap-5 items-start max-md:grid-cols-1">
      <div className="space-y-4.5">
        <Card>
          <CardHeader>
            <CardTitle className="text-xl tracking-tight">
              Manage Jetton
            </CardTitle>
            <CardDescription>
              Enter a Jetton minter contract address
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex gap-2.5">
              <Input
                type="text"
                placeholder="EQA... or 0:..."
                value={contractAddr}
                onChange={(e) => setContractAddr(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && loadJettonInfo()}
              />
              <Button
                className="rounded-full shrink-0 min-w-[80px]"
                style={{ background: '#0098EA' }}
                onClick={loadJettonInfo}
                disabled={loading}
              >
                {loading ? <span className="spinner" /> : 'Load'}
              </Button>
            </div>

            {status && !jettonInfo && status.type !== 'error' && (
              <StatusAlert
                type={status.type}
                message={status.message}
                className="mt-4"
              />
            )}
          </CardContent>
        </Card>

        {jettonInfo && (
          <Card>
            <CardContent>
              <Tabs
                value={tab}
                onValueChange={(v) => {
                  setTab(v as ManageTab);
                  setStatus(null);
                }}
              >
                <TabsList
                  className="w-full h-10 rounded-full p-[3px]"
                  style={{
                    background: theme === 'light' ? '#F0F1F3' : '#222224',
                  }}
                >
                  {(['mint', 'transfer', 'burn', 'admin'] as ManageTab[]).map(
                    (t) => (
                      <TabsTrigger
                        key={t}
                        value={t}
                        className="flex-1 h-[34px] rounded-full text-[13px] font-bold uppercase tracking-wider text-[#9a9a9f] hover:text-foreground data-[state=active]:bg-[#0098EA] data-[state=active]:text-white"
                      >
                        {t.charAt(0).toUpperCase() + t.slice(1)}
                      </TabsTrigger>
                    ),
                  )}
                </TabsList>

                <TabsContent value="mint" className="mt-5">
                  <MintTab
                    contractAddr={contractAddr}
                    decimals={decimals}
                    isAdmin={isAdmin}
                    isConnected={isConnected}
                    network={network}
                    tonConnectUI={tonConnectUI}
                    ownerAddress={ownerAddress}
                    onSuccess={loadJettonInfo}
                  />
                </TabsContent>
                <TabsContent value="transfer" className="mt-5">
                  <TransferTab
                    contractAddr={contractAddr}
                    decimals={decimals}
                    isConnected={isConnected}
                    network={network}
                    tonConnectUI={tonConnectUI}
                    ownerAddress={ownerAddress}
                  />
                </TabsContent>
                <TabsContent value="burn" className="mt-5">
                  <BurnTab
                    contractAddr={contractAddr}
                    decimals={decimals}
                    isConnected={isConnected}
                    network={network}
                    tonConnectUI={tonConnectUI}
                    ownerAddress={ownerAddress}
                    onSuccess={loadJettonInfo}
                  />
                </TabsContent>
                <TabsContent value="admin" className="mt-5">
                  <AdminTab
                    contractAddr={contractAddr}
                    info={jettonInfo}
                    isAdmin={isAdmin}
                    isConnected={isConnected}
                    network={network}
                    tonConnectUI={tonConnectUI}
                    onSuccess={loadJettonInfo}
                  />
                </TabsContent>
              </Tabs>
            </CardContent>
          </Card>
        )}
      </div>

      <JettonInfoCard
        info={jettonInfo}
        decimals={decimals}
        formatAmount={formatAmount}
        isAdmin={isAdmin}
        network={network}
        loading={loading}
        error={!jettonInfo && status?.type === 'error' ? status.message : null}
        contractAddr={contractAddr}
      />
    </div>
  );
}

function JettonInfoCard({
  info,
  decimals,
  formatAmount,
  isAdmin,
  network,
  loading,
  error,
  contractAddr,
}: {
  info: JettonInfo | null;
  decimals: number;
  formatAmount: (n: bigint) => string;
  isAdmin: boolean;
  network: 'mainnet' | 'testnet';
  loading: boolean;
  error: string | null;
  contractAddr: string;
}) {
  const [imgError, setImgError] = useState(false);

  if (loading) {
    return (
      <Card className="sticky top-20 max-md:static">
        <CardContent className="flex items-center justify-center min-h-[200px]">
          <span className="spinner" />
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card className="sticky top-20 max-md:static">
        <CardContent className="flex items-center justify-center min-h-[200px]">
          <EmptyState
            icon={<AlertCircle className="size-8" />}
            title="Jetton not found"
            description={error}
          />
        </CardContent>
      </Card>
    );
  }

  if (!info) {
    return (
      <Card className="sticky top-20 max-md:static">
        <CardContent className="flex items-center justify-center min-h-[200px]">
          <EmptyState
            icon={<Search className="size-8" />}
            title="Load a jetton"
            description="Enter an address and press Load to see token details"
          />
        </CardContent>
      </Card>
    );
  }

  const symbol = info.metadata.symbol || '???';
  const name = info.metadata.name || 'Unknown Token';
  const initial = symbol.charAt(0).toUpperCase();
  const imageUrl = info.metadata.image || '';

  return (
    <Card className="sticky top-20 max-md:static">
      <CardContent className="space-y-0">
        <div className="flex items-center gap-3.5 mb-5">
          <Avatar className="size-14 border-2 border-border">
            {imageUrl && !imgError ? (
              <AvatarImage
                src={imageUrl}
                alt={name}
                onError={() => setImgError(true)}
              />
            ) : null}
            <AvatarFallback className="bg-[#0098EA] text-white text-xl font-extrabold">
              {initial}
            </AvatarFallback>
          </Avatar>
          <div className="min-w-0">
            <div className="text-lg font-bold tracking-tight truncate">
              {name}
            </div>
            <div className="font-mono text-[13px] font-semibold text-[#0098EA]">
              ${symbol}
            </div>
          </div>
        </div>

        <Separator className="my-4" />

        <PreviewRow
          label="Supply"
          value={`${formatAmount(info.totalSupply)} ${symbol}`}
        />
        <PreviewRow label="Decimals" value={String(decimals)} />
        <PreviewRow label="Standard" value="TEP-74 Jetton" />
        <PreviewRow
          label="Mintable"
          value={info.mintable ? 'Yes' : 'No'}
          valueClassName={
            info.mintable ? 'text-[var(--success)]' : 'text-[var(--warning)]'
          }
        />
        <div className="flex justify-between items-center py-2">
          <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
            Admin
          </span>
          <span className="font-mono text-[13px] font-semibold text-right max-w-[65%] truncate">
            {info.adminAddress ? (
              <span className="inline-flex items-center gap-1.5">
                <AddressLink
                  address={info.adminAddress.toString({
                    bounceable: true,
                    testOnly: network === 'testnet',
                  })}
                  network={network}
                />
                {isAdmin && (
                  <Badge
                    variant="secondary"
                    className="text-[10px] bg-[var(--success)]/10 text-[var(--success)] border-0"
                  >
                    You
                  </Badge>
                )}
              </span>
            ) : (
              'None (revoked)'
            )}
          </span>
        </div>
        <div className="flex justify-between items-center py-2">
          <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
            Source
          </span>
          <span className="font-mono text-[13px] font-semibold text-right">
            <a
              href={`https://verifier.ton.org/${contractAddr.trim()}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#0098EA] hover:underline"
            >
              View on Verifier
            </a>
          </span>
        </div>

        {info.metadata.description && (
          <>
            <Separator className="my-4" />
            <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1.5">
              About
            </div>
            <p className="text-sm text-muted-foreground line-clamp-3 leading-relaxed">
              {info.metadata.description}
            </p>
          </>
        )}

        <NetworkBadge network={network} />
      </CardContent>
    </Card>
  );
}

function shortenAddress(addr: string): string {
  if (addr.length <= 12) return addr;
  return addr.slice(0, 4) + '...' + addr.slice(-4);
}

function AddressLink({
  address,
  network,
}: {
  address: string;
  network: 'mainnet' | 'testnet';
}) {
  const base =
    network === 'testnet'
      ? 'https://testnet.tonviewer.com'
      : 'https://tonviewer.com';
  return (
    <a
      href={`${base}/${address}`}
      target="_blank"
      rel="noopener noreferrer"
      title={address}
      className="text-[#0098EA] hover:underline"
    >
      {shortenAddress(address)}
    </a>
  );
}

function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="text-center py-8 px-4">
      <div className="mb-3.5 text-muted-foreground flex justify-center">
        {icon}
      </div>
      <div className="text-[15px] font-semibold mb-1.5">{title}</div>
      {description && (
        <p className="text-sm text-muted-foreground mb-4.5 leading-relaxed">
          {description}
        </p>
      )}
      {action}
    </div>
  );
}

function WalletRequired({ tonConnectUI }: { tonConnectUI: any }) {
  return (
    <EmptyState
      icon={<Wallet className="size-8" />}
      title="Wallet not connected"
      description="Connect your wallet to perform this action"
      action={
        <Button
          className="rounded-full max-w-[220px] mx-auto"
          onClick={() => tonConnectUI.openModal()}
        >
          Connect Wallet
        </Button>
      }
    />
  );
}

function AdminRequired() {
  return (
    <EmptyState
      icon={<Lock className="size-8" />}
      title="Admin access required"
      description="Only the contract admin can perform this action"
    />
  );
}

function MintTab({
  contractAddr,
  decimals,
  isAdmin,
  isConnected,
  network,
  tonConnectUI,
  ownerAddress,
  onSuccess,
}: {
  contractAddr: string;
  decimals: number;
  isAdmin: boolean;
  isConnected: boolean;
  network: 'mainnet' | 'testnet';
  tonConnectUI: any;
  ownerAddress: Address | null;
  onSuccess: () => void;
}) {
  const [toAddr, setToAddr] = useState('');
  const [amount, setAmount] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{
    type: string;
    message: string;
  } | null>(null);

  if (!isConnected) return <WalletRequired tonConnectUI={tonConnectUI} />;
  if (!isAdmin) return <AdminRequired />;

  async function handleMint(e: FormEvent) {
    e.preventDefault();
    if (!ownerAddress) return;

    const recipientRaw = toAddr.trim() || ownerAddress.toString();
    const recipientAddr = tryParseAddress(recipientRaw);
    if (!recipientAddr) {
      setStatus({ type: 'error', message: 'Invalid recipient address' });
      return;
    }
    const amountParsed = parseFloat(amount);
    if (isNaN(amountParsed) || amountParsed <= 0) {
      setStatus({ type: 'error', message: 'Enter a valid amount' });
      return;
    }

    setLoading(true);
    setStatus({ type: 'info', message: 'Confirm in your wallet...' });

    try {
      const mintAmountNano = parseUnits(amount.trim(), decimals);
      const body = buildMintBody({
        toAddress: recipientAddr,
        jettonAmount: mintAmountNano,
        forwardTonAmount: toNano('0.02'),
        totalTonAmount: toNano('0.05'),
      });

      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: Address.parse(contractAddr).toString(),
            amount: toNano('0.1').toString(),
            payload: body.toBoc().toString('base64'),
          },
        ],
      });

      setStatus({ type: 'success', message: 'Mint transaction sent!' });
      setAmount('');
      setTimeout(onSuccess, 5000);
    } catch (err: any) {
      const msg = err?.message || '';
      setStatus({
        type: 'error',
        message: msg.match(/cancel|reject|closed|Interrupt/i)
          ? 'Transaction cancelled'
          : msg || 'Mint failed',
      });
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }

  return (
    <form onSubmit={handleMint} className="space-y-4.5">
      <div className="space-y-1.5">
        <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          Recipient Address
        </Label>
        <Input
          type="text"
          placeholder="Leave empty to mint to yourself"
          value={toAddr}
          onChange={(e) => setToAddr(e.target.value)}
          disabled={loading}
        />
      </div>
      <div className="space-y-1.5">
        <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          Amount
        </Label>
        <Input
          type="text"
          placeholder="1000"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          disabled={loading}
        />
      </div>
      <Button
        className="w-full h-12 rounded-full text-[15px] font-bold"
        disabled={loading}
      >
        {loading ? (
          <>
            <span className="spinner" /> Minting...
          </>
        ) : (
          'Mint Tokens'
        )}
      </Button>
      {status && <StatusAlert type={status.type} message={status.message} />}
    </form>
  );
}

function TransferTab({
  contractAddr,
  decimals,
  isConnected,
  network,
  tonConnectUI,
  ownerAddress,
}: {
  contractAddr: string;
  decimals: number;
  isConnected: boolean;
  network: 'mainnet' | 'testnet';
  tonConnectUI: any;
  ownerAddress: Address | null;
}) {
  const [toAddr, setToAddr] = useState('');
  const [amount, setAmount] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{
    type: string;
    message: string;
  } | null>(null);

  if (!isConnected) return <WalletRequired tonConnectUI={tonConnectUI} />;

  async function handleTransfer(e: FormEvent) {
    e.preventDefault();
    if (!ownerAddress) return;

    const recipientAddr = tryParseAddress(toAddr);
    if (!recipientAddr) {
      setStatus({ type: 'error', message: 'Invalid recipient address' });
      return;
    }
    const amountParsed = parseFloat(amount);
    if (isNaN(amountParsed) || amountParsed <= 0) {
      setStatus({ type: 'error', message: 'Enter a valid amount' });
      return;
    }

    setLoading(true);
    setStatus({ type: 'info', message: 'Confirm in your wallet...' });

    try {
      const transferAmount = parseUnits(amount.trim(), decimals);
      const body = buildTransferBody({
        toAddress: recipientAddr,
        amount: transferAmount,
        responseAddress: ownerAddress,
        forwardTonAmount: toNano('0.001'),
      });

      const client = getTonClient(network);
      const walletAddr = await getWalletAddress(
        client,
        Address.parse(contractAddr),
        ownerAddress,
      );

      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: walletAddr.toString(),
            amount: toNano('0.05').toString(),
            payload: body.toBoc().toString('base64'),
          },
        ],
      });

      setStatus({ type: 'success', message: 'Transfer transaction sent!' });
      setAmount('');
      setToAddr('');
    } catch (err: any) {
      const msg = err?.message || '';
      setStatus({
        type: 'error',
        message: msg.match(/cancel|reject|closed|Interrupt/i)
          ? 'Transaction cancelled'
          : msg || 'Transfer failed',
      });
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }

  return (
    <form onSubmit={handleTransfer} className="space-y-4.5">
      <div className="space-y-1.5">
        <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          Recipient Address
        </Label>
        <Input
          type="text"
          placeholder="EQA..."
          value={toAddr}
          onChange={(e) => setToAddr(e.target.value)}
          disabled={loading}
        />
      </div>
      <div className="space-y-1.5">
        <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          Amount
        </Label>
        <Input
          type="text"
          placeholder="100"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          disabled={loading}
        />
      </div>
      <Button
        className="w-full h-12 rounded-full text-[15px] font-bold"
        disabled={loading}
      >
        {loading ? (
          <>
            <span className="spinner" /> Transferring...
          </>
        ) : (
          'Transfer Tokens'
        )}
      </Button>
      {status && <StatusAlert type={status.type} message={status.message} />}
    </form>
  );
}

function BurnTab({
  contractAddr,
  decimals,
  isConnected,
  network,
  tonConnectUI,
  ownerAddress,
  onSuccess,
}: {
  contractAddr: string;
  decimals: number;
  isConnected: boolean;
  network: 'mainnet' | 'testnet';
  tonConnectUI: any;
  ownerAddress: Address | null;
  onSuccess: () => void;
}) {
  const [amount, setAmount] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{
    type: string;
    message: string;
  } | null>(null);

  if (!isConnected) return <WalletRequired tonConnectUI={tonConnectUI} />;

  async function handleBurn(e: FormEvent) {
    e.preventDefault();
    if (!ownerAddress) return;

    const amountParsed = parseFloat(amount);
    if (isNaN(amountParsed) || amountParsed <= 0) {
      setStatus({ type: 'error', message: 'Enter a valid amount' });
      return;
    }

    setLoading(true);
    setStatus({ type: 'info', message: 'Confirm in your wallet...' });

    try {
      const burnAmount = parseUnits(amount.trim(), decimals);
      const body = buildBurnBody(burnAmount, ownerAddress);

      const client = getTonClient(network);
      const walletAddr = await getWalletAddress(
        client,
        Address.parse(contractAddr),
        ownerAddress,
      );

      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: walletAddr.toString(),
            amount: toNano('0.05').toString(),
            payload: body.toBoc().toString('base64'),
          },
        ],
      });

      setStatus({ type: 'success', message: 'Burn transaction sent!' });
      setAmount('');
      setTimeout(onSuccess, 5000);
    } catch (err: any) {
      const msg = err?.message || '';
      setStatus({
        type: 'error',
        message: msg.match(/cancel|reject|closed|Interrupt/i)
          ? 'Transaction cancelled'
          : msg || 'Burn failed',
      });
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }

  return (
    <form onSubmit={handleBurn} className="space-y-4.5">
      <div className="space-y-1.5">
        <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          Amount to Burn
        </Label>
        <Input
          type="text"
          placeholder="1000"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          disabled={loading}
        />
        <p className="text-xs text-muted-foreground">
          Burns tokens from your wallet. This action is irreversible.
        </p>
      </div>
      <Button
        variant="destructive"
        className="w-full h-12 rounded-full text-[15px] font-bold"
        disabled={loading}
      >
        {loading ? (
          <>
            <span className="spinner" /> Burning...
          </>
        ) : (
          'Burn Tokens'
        )}
      </Button>
      {status && <StatusAlert type={status.type} message={status.message} />}
    </form>
  );
}

function AdminTab({
  contractAddr,
  info,
  isAdmin,
  isConnected,
  network,
  tonConnectUI,
  onSuccess,
}: {
  contractAddr: string;
  info: JettonInfo;
  isAdmin: boolean;
  isConnected: boolean;
  network: 'mainnet' | 'testnet';
  tonConnectUI: any;
  onSuccess: () => void;
}) {
  const [newAdmin, setNewAdmin] = useState(ZERO_ADDRESS);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{
    type: string;
    message: string;
  } | null>(null);

  const [newName, setNewName] = useState(info.metadata.name || '');
  const [newSymbol, setNewSymbol] = useState(info.metadata.symbol || '');
  const [newDecimals, setNewDecimals] = useState(info.metadata.decimals || '9');
  const [newDescription, setNewDescription] = useState(
    info.metadata.description || '',
  );
  const [newImage, setNewImage] = useState(info.metadata.image || '');

  if (!isConnected) return <WalletRequired tonConnectUI={tonConnectUI} />;
  if (!isAdmin) return <AdminRequired />;

  async function handleChangeAdmin(e: FormEvent) {
    e.preventDefault();
    const adminAddr = tryParseAddress(newAdmin);
    if (!adminAddr) {
      setStatus({ type: 'error', message: 'Invalid admin address' });
      return;
    }

    setLoading(true);
    setStatus({ type: 'info', message: 'Confirm in your wallet...' });

    try {
      const body = buildChangeAdminBody(adminAddr);
      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: Address.parse(contractAddr).toString(),
            amount: toNano('0.05').toString(),
            payload: body.toBoc().toString('base64'),
          },
        ],
      });
      setStatus({ type: 'success', message: 'Admin change transaction sent!' });
      setTimeout(onSuccess, 5000);
    } catch (err: any) {
      const msg = err?.message || '';
      setStatus({
        type: 'error',
        message: msg.match(/cancel|reject|closed|Interrupt/i)
          ? 'Transaction cancelled'
          : msg || 'Failed',
      });
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }

  async function handleUpdateContent(e: FormEvent) {
    e.preventDefault();
    setLoading(true);
    setStatus({ type: 'info', message: 'Confirm in your wallet...' });

    try {
      const body = await buildChangeContentBody({
        name: newName,
        symbol: newSymbol,
        decimals: newDecimals,
        description: newDescription || undefined,
        image: newImage || undefined,
      });
      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: Address.parse(contractAddr).toString(),
            amount: toNano('0.05').toString(),
            payload: body.toBoc().toString('base64'),
          },
        ],
      });
      setStatus({
        type: 'success',
        message: 'Content update transaction sent!',
      });
      setTimeout(onSuccess, 5000);
    } catch (err: any) {
      const msg = err?.message || '';
      setStatus({
        type: 'error',
        message: msg.match(/cancel|reject|closed|Interrupt/i)
          ? 'Transaction cancelled'
          : msg || 'Failed',
      });
    } finally {
      setLoading(false);
      setStatus((prev: any) => (prev?.type === 'info' ? null : prev));
    }
  }

  return (
    <div className="space-y-0">
      <form onSubmit={handleUpdateContent} className="space-y-4.5">
        <h3 className="text-base font-semibold">Update Metadata</h3>
        <div className="grid grid-cols-2 gap-3.5 max-sm:grid-cols-1">
          <div className="space-y-1.5">
            <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
              Name
            </Label>
            <Input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
              Symbol
            </Label>
            <Input
              type="text"
              value={newSymbol}
              onChange={(e) => setNewSymbol(e.target.value)}
              disabled={loading}
            />
          </div>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
            Description
          </Label>
          <Textarea
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            disabled={loading}
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
            Image URL
          </Label>
          <Input
            type="text"
            value={newImage}
            onChange={(e) => setNewImage(e.target.value)}
            disabled={loading}
          />
        </div>
        <Button
          className="w-full h-12 rounded-full text-[15px] font-bold"
          disabled={loading}
        >
          {loading ? (
            <>
              <span className="spinner" /> Updating...
            </>
          ) : (
            'Update Metadata'
          )}
        </Button>
      </form>

      <Separator className="my-6" />

      <form onSubmit={handleChangeAdmin} className="space-y-4.5">
        <h3 className="text-base font-semibold">Transfer Admin</h3>
        <div className="space-y-1.5">
          <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
            New Admin Address
          </Label>
          <Input
            type="text"
            placeholder="EQA..."
            value={newAdmin}
            onChange={(e) => setNewAdmin(e.target.value)}
            disabled={loading}
          />
          <p className="text-xs text-muted-foreground">
            Zero address (0:000...0) revokes admin rights permanently
          </p>
        </div>
        <Button
          variant="destructive"
          className="w-full h-12 rounded-full text-[15px] font-bold"
          disabled={loading}
        >
          {loading ? (
            <>
              <span className="spinner" /> Transferring...
            </>
          ) : (
            'Transfer Admin Rights'
          )}
        </Button>
      </form>

      {status && (
        <StatusAlert
          type={status.type}
          message={status.message}
          className="mt-4"
        />
      )}
    </div>
  );
}
