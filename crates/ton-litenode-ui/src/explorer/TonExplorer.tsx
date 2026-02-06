import React, { useState, useEffect } from "react";
import { Address } from "@ton/core";
import { TonClient } from "./api/client";
import { FullAccountState, Transaction } from "./types";
import { AccountInfo } from "./components/AccountInfo";
import { TransactionList } from "./components/TransactionList";
import { Breadcrumbs } from "./components/Breadcrumbs";
import styles from "./TonExplorer.module.css";

interface TonExplorerProps {
  client: TonClient;
  externalAddress?: string;
  onAddressChange?: (addr: string) => void;
}

export const TonExplorer: React.FC<TonExplorerProps> = ({ 
  client, 
  externalAddress = "",
  onAddressChange 
}) => {
  const [accountState, setAccountState] = useState<FullAccountState | null>(null);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async (addr: string) => {
    if (!addr) {
      setAccountState(null);
      setTransactions([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [state, txs] = await Promise.all([
        client.getAddressInformation(addr),
        client.getTransactions(addr),
      ]);
      setAccountState(state);
      setTransactions(txs);
    } catch (e) {
      setError(e instanceof Error ? e.message : "An error occurred");
      setAccountState(null);
      setTransactions([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData(externalAddress);
  }, [externalAddress]);

  return (
    <div className={styles.container}>
      {loading && (
        <div className={styles.loading}>Loading...</div>
      )}

      {error && (
        <div className={styles.error}>
          {error}
        </div>
      )}

      {accountState && !loading && (
        <>
          <Breadcrumbs 
            items={[
              { 
                label: externalAddress ? Address.parse(externalAddress).toString({ testOnly: true }) : "", 
                isAddress: true 
              }
            ]} 
          />
          <AccountInfo address={externalAddress} state={accountState} />
          <TransactionList 
            transactions={transactions} 
            accountState={accountState}
            ownerAddress={externalAddress}
            onAddressClick={onAddressChange}
          />
        </>
      )}

      {!accountState && !loading && !error && externalAddress && (
        <div className={styles.empty}>
          No data found for this address.
        </div>
      )}
    </div>
  );
};
