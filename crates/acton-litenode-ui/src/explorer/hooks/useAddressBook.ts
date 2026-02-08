import { Address } from "@ton/core";
import type React from "react";
import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { TonClient } from "../api/client";

type AddressName = string | undefined;

interface AddressBookContextValue {
  readonly getCachedName: (address: string) => AddressName | undefined;
  readonly fetchName: (address: string) => Promise<AddressName>;
  readonly updateName: (address: string, name: AddressName) => void;
  readonly setAddressName: (address: string, name: string) => Promise<void>;
  readonly version: number;
}

const AddressBookContext = createContext<AddressBookContextValue | undefined>(undefined);

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
      updateName(address, name || undefined);
    },
    [client, updateName],
  );

  const fetchName = useCallback(
    async (address: string) => {
      if (!address) return;
      const key = normalizeKey(address);
      if (cacheRef.current.has(key)) {
        return cacheRef.current.get(key);
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
          updateName(address, undefined as AddressName)
          return;
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

  return createElement(
    AddressBookContext.Provider,
    { value },
    children,
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
    () => getCachedName(address),
  );

  useEffect(() => {
    const cached = getCachedName(address);
    if (cached !== undefined) {
      setName(cached);
    }
  }, [address, getCachedName, version]);

  useEffect(() => {
    if (!address) {
      setName(undefined);
      return;
    }
    let isActive = true;
    const cached = getCachedName(address);
    if (cached === undefined) {
      void fetchName(address).then((next) => {
        if (isActive) setName(next);
      });
    }
    return () => {
      isActive = false;
    };
  }, [address, fetchName, getCachedName]);

  return name;
};
