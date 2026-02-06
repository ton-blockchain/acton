import { Address } from "@ton/core";

export function formatNano(nano: string | number): string {
  const n = typeof nano === "string" ? BigInt(nano) : BigInt(nano);
  const ton = Number(n) / 1e9;
  return ton.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 9 });
}

export function formatTime(utime: number): string {
  const date = new Date(utime * 1000);
  return date.toLocaleString();
}

export function formatTimeAgo(utime: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - utime;

  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  
  const date = new Date(utime * 1000);
  const day = date.getDate();
  const month = date.toLocaleString('default', { month: 'short' });
  const time = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  return `${day} ${month}, ${time}`;
}

export function formatAddress(address: string, shorten: boolean = true): string {
  if (!address) return "Unknown";
  
  let displayAddress = address;
  try {
    // If it's a hex address (with or without workchain), convert to user-friendly base64
    if (address.includes(':') || /^[0-9a-fA-F]+$/.test(address)) {
      displayAddress = Address.parse(address).toString({ testOnly: true });
    } else {
      // Re-parse base64 to ensure it's in testnet format if it's not already
      displayAddress = Address.parse(address).toString({ testOnly: true });
    }
  } catch (e) {
    // If parsing fails, use original address
  }

  if (!shorten) return displayAddress;

  // Check if it's a workchain:address format
  if (displayAddress.includes(':')) {
    const [workchain, hash] = displayAddress.split(':');
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`;
  }
  // Standard base64 address
  if (displayAddress.length > 16) {
    return `${displayAddress.slice(0, 8)}…${displayAddress.slice(-8)}`;
  }
  return displayAddress;
}
