import { useState, type FormEvent } from 'react';
import { useTonConnectUI, useTonWallet } from '@tonconnect/ui-react';
import { Address, toNano, beginCell, storeStateInit } from '@ton/core';
import { CheckCircle, ExternalLink, Copy, AlertCircle, Info, CheckCircle2 } from 'lucide-react';
import { buildDeployMessage, parseUnits } from '../lib/deploy';
import { Button } from '@/components/ui/button';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertTitle } from '@/components/ui/alert';
import { Avatar, AvatarImage, AvatarFallback } from '@/components/ui/avatar';

interface Props {
  network: 'mainnet' | 'testnet';
}

export function DeployPage({ network }: Props) {
  const [tonConnectUI] = useTonConnectUI();
  const wallet = useTonWallet();

  const [name, setName] = useState('Acton Token');
  const [symbol, setSymbol] = useState('ACT');
  const [decimals, setDecimals] = useState('9');
  const [description, setDescription] = useState('A sample token on the TON blockchain');
  const [imageUrl, setImageUrl] = useState('');
  const [mintAmount, setMintAmount] = useState('1000000');

  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{
    type: 'success' | 'error' | 'info';
    message: string;
  } | null>(null);
  const [deployedAddress, setDeployedAddress] = useState<string | null>(null);

  const ownerAddress = wallet?.account?.address ? Address.parse(wallet.account.address) : null;

  const isConnected = !!wallet;

  async function handleDeploy(e: FormEvent) {
    e.preventDefault();

    if (!isConnected) {
      tonConnectUI.openModal();
      return;
    }

    if (!ownerAddress) return;

    if (!name.trim() || !symbol.trim()) {
      setStatus({ type: 'error', message: 'Name and symbol are required' });
      return;
    }

    const dec = parseInt(decimals);
    if (isNaN(dec) || dec < 0 || dec > 18) {
      setStatus({ type: 'error', message: 'Decimals must be between 0 and 18' });
      return;
    }

    const mintAmountParsed = parseFloat(mintAmount);
    if (isNaN(mintAmountParsed) || mintAmountParsed <= 0) {
      setStatus({ type: 'error', message: 'Enter a valid mint amount' });
      return;
    }

    setLoading(true);
    setStatus({ type: 'info', message: 'Preparing deployment...' });

    try {
      const mintAmountNano = parseUnits(mintAmount.trim(), dec);

      const { contractAddress, stateInit, mintBody } = await buildDeployMessage({
        metadata: {
          name: name.trim(),
          symbol: symbol.trim(),
          decimals: decimals,
          description: description.trim() || undefined,
          image: imageUrl.trim() || undefined,
        },
        ownerAddress,
        mintAmount: mintAmountNano,
      });

      setStatus({ type: 'info', message: 'Confirm the transaction in your wallet...' });

      await tonConnectUI.sendTransaction({
        validUntil: Math.floor(Date.now() / 1000) + 300,
        network: network === 'mainnet' ? '-239' : '-3',
        messages: [
          {
            address: contractAddress.toString(),
            amount: toNano('0.15').toString(),
            stateInit: beginCell()
              .store(storeStateInit(stateInit))
              .endCell()
              .toBoc()
              .toString('base64'),
            payload: mintBody.toBoc().toString('base64'),
          },
        ],
      });

      const friendlyAddr = contractAddress.toString({
        bounceable: true,
        testOnly: network === 'testnet',
      });
      setDeployedAddress(friendlyAddr);
      setStatus({ type: 'success', message: 'Jetton deployed successfully!' });
    } catch (err: any) {
      const msg = err?.message || String(err) || '';
      if (
        msg.includes('Interrupted') ||
        msg.includes('cancel') ||
        msg.includes('reject') ||
        msg.includes('Cancelled') ||
        msg.includes('closed')
      ) {
        setStatus({ type: 'error', message: 'Transaction cancelled' });
      } else {
        setStatus({ type: 'error', message: msg || 'Deployment failed' });
      }
    } finally {
      setLoading(false);
      setStatus((prev) => (prev?.type === 'info' ? null : prev));
    }
  }

  const displaySymbol = symbol.trim() || 'TKN';
  const displayName = name.trim() || 'Token Name';

  return (
    <div className="grid grid-cols-[1fr_320px] gap-5 items-start max-md:grid-cols-1">
      <div className="space-y-4.5">
        <Card>
          <CardHeader>
            <CardTitle className="text-xl tracking-tight">Deploy New Jetton</CardTitle>
            <CardDescription>
              Create a new Jetton token on{' '}
              {network === 'mainnet' ? 'TON Mainnet' : 'TON Testnet'}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleDeploy} className="space-y-4.5">
              <div className="grid grid-cols-2 gap-3.5 max-sm:grid-cols-1">
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                    Token Name
                  </Label>
                  <Input
                    placeholder="My Token"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    disabled={loading}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                    Symbol
                  </Label>
                  <Input
                    placeholder="MTK"
                    value={symbol}
                    onChange={(e) => setSymbol(e.target.value)}
                    disabled={loading}
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3.5 max-sm:grid-cols-1">
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                    Decimals
                  </Label>
                  <Input
                    type="number"
                    min="0"
                    max="18"
                    value={decimals}
                    onChange={(e) => setDecimals(e.target.value)}
                    disabled={loading}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                    Initial Supply
                  </Label>
                  <Input
                    placeholder="1000000"
                    value={mintAmount}
                    onChange={(e) => setMintAmount(e.target.value)}
                    disabled={loading}
                  />
                  <p className="text-xs text-muted-foreground">Minted to your wallet</p>
                </div>
              </div>

              <div className="space-y-1.5">
                <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                  Description
                </Label>
                <Textarea
                  placeholder="Describe your token..."
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  disabled={loading}
                />
              </div>

              <div className="space-y-1.5">
                <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                  Image URL
                </Label>
                <Input
                  type="text"
                  placeholder="https://example.com/logo.png"
                  value={imageUrl}
                  onChange={(e) => setImageUrl(e.target.value)}
                  disabled={loading}
                />
                <p className="text-xs text-muted-foreground">PNG recommended, 256x256px</p>
              </div>

              <Button
                className="w-full h-12 rounded-full text-[15px] font-bold"
                disabled={loading}
              >
                {loading ? (
                  <>
                    <span className="spinner" /> Deploying...
                  </>
                ) : !isConnected ? (
                  'Connect Wallet to Deploy'
                ) : (
                  'Deploy Jetton'
                )}
              </Button>
            </form>

            {status && !deployedAddress && (
              <StatusAlert type={status.type} message={status.message} className="mt-4" />
            )}
          </CardContent>
        </Card>

        {deployedAddress && <DeployedCard address={deployedAddress} network={network} />}
      </div>

      <TokenPreview
        name={displayName}
        symbol={displaySymbol}
        decimals={decimals}
        supply={mintAmount}
        description={description}
        imageUrl={imageUrl}
        network={network}
      />
    </div>
  );
}

export function StatusAlert({
  type,
  message,
  className,
}: {
  type: string;
  message: string;
  className?: string;
}) {
  const variant = type === 'error' ? 'destructive' : type === 'success' ? 'success' : 'info';
  const Icon = type === 'error' ? AlertCircle : type === 'success' ? CheckCircle2 : Info;
  return (
    <Alert variant={variant} className={className}>
      <Icon className="size-4" />
      <AlertTitle>{message}</AlertTitle>
    </Alert>
  );
}

function DeployedCard({
  address,
  network,
}: {
  address: string;
  network: 'mainnet' | 'testnet';
}) {
  const [copied, setCopied] = useState(false);
  const base =
    network === 'testnet' ? 'https://testnet.tonviewer.com' : 'https://tonviewer.com';

  return (
    <Card>
      <CardContent className="text-center py-5">
        <div className="mb-3.5 flex justify-center" style={{ color: 'var(--success)' }}>
          <CheckCircle className="size-9" strokeWidth={1.5} />
        </div>
        <div className="text-base font-bold mb-1.5">Jetton Deployed</div>
        <p className="text-sm text-muted-foreground mb-4.5">
          Your contract is live on {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
        </p>
        <div className="flex items-center justify-center gap-2.5 flex-wrap">
          <Button asChild className="rounded-full h-10">
            <a href={`${base}/${address}`} target="_blank" rel="noopener noreferrer">
              <ExternalLink className="size-4" />
              View on Tonviewer
            </a>
          </Button>
          <Button
            variant="secondary"
            className="rounded-full h-10"
            onClick={() => {
              navigator.clipboard.writeText(address);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >
            <Copy className="size-4" />
            {copied ? 'Copied!' : 'Copy Address'}
          </Button>
        </div>
        <p className="mt-3.5 font-mono text-xs text-muted-foreground break-all">{address}</p>
      </CardContent>
    </Card>
  );
}

function TokenPreview({
  name,
  symbol,
  decimals,
  supply,
  description,
  imageUrl,
  network,
}: {
  name: string;
  symbol: string;
  decimals: string;
  supply: string;
  description: string;
  imageUrl: string;
  network: 'mainnet' | 'testnet';
}) {
  const [imgError, setImgError] = useState(false);
  const initial = symbol.charAt(0).toUpperCase();

  const dec = parseInt(decimals) || 9;
  const supplyNum = parseFloat(supply);
  const formattedSupply = !isNaN(supplyNum)
    ? supplyNum.toLocaleString('en-US', { maximumFractionDigits: 0 })
    : '0';

  return (
    <Card className="sticky top-20 max-md:static max-md:order-[-1]">
      <CardContent className="space-y-0">
        <div className="flex items-center gap-3.5 mb-5">
          <Avatar className="size-14 border-2 border-border">
            {imageUrl.trim() && !imgError ? (
              <AvatarImage
                src={imageUrl.trim()}
                alt={name}
                onError={() => setImgError(true)}
              />
            ) : null}
            <AvatarFallback className="bg-[#0098EA] text-white text-xl font-extrabold">
              {initial}
            </AvatarFallback>
          </Avatar>
          <div className="min-w-0">
            <div className="text-lg font-bold tracking-tight truncate">{name}</div>
            <div className="font-mono text-[13px] font-semibold text-[#0098EA]">${symbol}</div>
          </div>
        </div>

        <Separator className="my-4" />

        <PreviewRow label="Supply" value={`${formattedSupply} ${symbol}`} />
        <PreviewRow label="Decimals" value={String(dec)} />
        <PreviewRow label="Standard" value="TEP-74 Jetton" />
        <PreviewRow label="Mintable" value="Yes" valueClassName="text-[var(--success)]" />

        {description.trim() && (
          <>
            <Separator className="my-4" />
            <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1.5">
              About
            </div>
            <p className="text-sm text-muted-foreground line-clamp-3 leading-relaxed">
              {description.trim()}
            </p>
          </>
        )}

        <NetworkBadge network={network} />
      </CardContent>
    </Card>
  );
}

export function PreviewRow({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: string;
  valueClassName?: string;
}) {
  return (
    <div className="flex justify-between items-center py-2">
      <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
        {label}
      </span>
      <span
        className={`font-mono text-[13px] font-semibold text-right max-w-[65%] truncate ${valueClassName || ''}`}
      >
        {value}
      </span>
    </div>
  );
}

export function NetworkBadge({ network }: { network: 'mainnet' | 'testnet' }) {
  return (
    <Badge
      variant="outline"
      className={`mt-4 gap-1.5 py-1 px-2.5 text-[11px] font-semibold uppercase tracking-wider ${
        network === 'mainnet'
          ? 'border-[var(--success)]/20 text-[var(--success)] bg-[var(--success)]/10'
          : 'border-[var(--warning)]/20 text-[var(--warning)] bg-[var(--warning)]/10'
      }`}
    >
      <span
        className="size-1.5 rounded-full"
        style={{
          background: network === 'mainnet' ? 'var(--success)' : 'var(--warning)',
        }}
      />
      {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
    </Badge>
  );
}
