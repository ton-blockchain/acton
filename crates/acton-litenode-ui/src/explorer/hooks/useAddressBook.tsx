import { Address } from "@ton/core";
import type React from "react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { TonClient } from "../api/client";

type AddressName = string | null;

interface AddressBookContextValue {
  readonly getCachedName: (address: string) => AddressName | undefined;
  readonly fetchName: (address: string) => Promise<AddressName>;
  readonly updateName: (address: string, name: AddressName) => void;
  readonly setAddressName: (address: string, name: string) => Promise<void>;
  readonly version: number;
}

const AddressBookContext = createContext<AddressBookContextValue | null>(null);

const normalizeKey = (address: string) => {
  try {
    return Address.parse(address).toRawString();
  } catch {
    return address;
  }
};

export const AddressBookProvider: React.FC<{
  client: TonClient;
  children: React.ReactNode;
}> = ({ client, children }) => {
  const cacheRef = useRef(new Map<string, AddressName>());
  const pendingRef = useRef(new Map<string, Promise<AddressName>>());
  const [version, setVersion] = useState(0);

  const getCachedName = useCallback((address: string) => {
    if (!address) return;
    const key = normalizeKey(address);
    return cacheRef.current.get(key);
  }, []);

  const updateName = useCallback((address: string, name: AddressName) => {
    if (!address) return;
    const key = normalizeKey(address);
    cacheRef.current.set(key, name);
    setVersion((prev) => prev + 1);
  }, []);

  const setAddressName = useCallback(
    async (address: string, name: string) => {
      await client.setAddressName(address, name);
      updateName(address, name || null);
    },
    [client, updateName],
  );

  const fetchName = useCallback(
    async (address: string) => {
      if (!address) return null;
      const key = normalizeKey(address);
      if (cacheRef.current.has(key)) {
        return cacheRef.current.get(key) ?? null;
      }
      const pending = pendingRef.current.get(key);
      if (pending) return pending;

      const request = (async () => {
        try {
          const name = await client.getAddressName(address);
          updateName(address, name);
          return name;
        } catch (error) {
          console.warn(`Failed to fetch name for ${address}:`, error);
          updateName(address, null);
          return null;
        } finally {
          pendingRef.current.delete(key);
        }
      })();

      pendingRef.current.set(key, request);
      return request;
    },
    [client, updateName],
  );

  const value = useMemo(
    () => ({
      getCachedName,
      fetchName,
      updateName,
      setAddressName,
      version,
    }),
    [fetchName, getCachedName, setAddressName, updateName, version],
  );

  return (
    <AddressBookContext.Provider value={value}>
      {children}
    </AddressBookContext.Provider>
  );
};

export const useAddressBook = () => {
  const ctx = useContext(AddressBookContext);
  if (!ctx) {
    throw new Error("useAddressBook must be used within AddressBookProvider");
  }
  return ctx;
};

export const useAddressName = (address: string) => {
  const { getCachedName, fetchName, version } = useAddressBook();
  const [name, setName] = useState<AddressName>(
    () => getCachedName(address) ?? null,
  );

  useEffect(() => {
    const cached = getCachedName(address);
    if (cached !== undefined) {
      setName(cached);
    }
  }, [address, getCachedName, version]);

  useEffect(() => {
    if (!address) {
      setName(null);
      return;
    }
    let isActive = true;
    const cached = getCachedName(address);
    if (cached === undefined) {
      fetchName(address).then((next) => {
        if (isActive) setName(next);
      });
    }
    return () => {
      isActive = false;
    };
  }, [address, fetchName, getCachedName]);

  return name;
};
