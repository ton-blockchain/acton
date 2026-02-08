import { Address } from "@ton/core";

// Global cache for address names to avoid flickering and repeated API calls
const addressNameCache: Record<string, string | null> = {};
const pendingRequests: Record<string, Promise<string | null>> = {};

export let tonClientInstance: any = null;

export function setTonClientInstance(client: any) {
  tonClientInstance = client;
}

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

export function getCachedAddressName(address: string): string | null {
  if (!address) return null;
  try {
    const stdAddr = Address.parse(address).toRawString();
    return addressNameCache[stdAddr] || null;
  } catch {
    return addressNameCache[address] || null;
  }
}

export async function fetchAddressName(address: string): Promise<string | null> {
  if (!address || !tonClientInstance) return null;
  
  let stdAddr: string;
  try {
    stdAddr = Address.parse(address).toRawString();
  } catch {
    stdAddr = address;
  }
  
  if (addressNameCache[stdAddr] !== undefined) return addressNameCache[stdAddr];
  if (pendingRequests[stdAddr]) return pendingRequests[stdAddr];
  
  pendingRequests[stdAddr] = (async () => {
    try {
      const name = await tonClientInstance.getAddressName(address);
      addressNameCache[stdAddr] = name;
      return name;
    } catch (e) {
      console.warn(`Failed to fetch name for ${address}:`, e);
      addressNameCache[stdAddr] = null; // Cache failure to avoid repeated requests
      return null;
    } finally {
      delete pendingRequests[stdAddr];
    }
  })();
  
  return pendingRequests[stdAddr];
}

export function updateCachedAddressName(address: string, name: string | null) {
  try {
    const stdAddr = Address.parse(address).toRawString();
    addressNameCache[stdAddr] = name;
  } catch {
    addressNameCache[address] = name;
  }
}

export function formatAddress(address: string, shorten: boolean = true, forceReal: boolean = false): string {
  if (!address) return "Unknown";
  
  // Try to use cached name first
  if (!forceReal) {
    const cachedName = getCachedAddressName(address);
    if (cachedName) return cachedName;
  }
  
  let displayAddress = address;
  try {
    displayAddress = Address.parse(address).toString({ testOnly: true });
  } catch (e) {
    // If parsing fails, use original address
  }

  if (!shorten) return displayAddress;

  // Standard base64 address or workchain:address format
  if (displayAddress.includes(':')) {
    const [workchain, hash] = displayAddress.split(':');
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`;
  }
  
  if (displayAddress.length > 12) {
    return `${displayAddress.slice(0, 6)}…${displayAddress.slice(-6)}`;
  }
  return displayAddress;
}
