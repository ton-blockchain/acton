import React, { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { TonClient } from "../api/client";
import {
  TransactionTree,
  processTransactions,
  type TransactionInfo,
  type ContractData,
  type BackendTransaction,
  fmt
} from "@acton/shared-ui";
import { Address } from "@ton/core";
import { ArrowLeft, Loader2, AlertCircle } from "lucide-react";
import styles from "./TransactionPage.module.css";

interface TransactionPageProps {
  client: TonClient;
}

// Интерфейсы для соответствия V3 API Response согласно OpenAPI
interface V3Transaction {
  hash: string;
  lt: string;
  raw_transaction?: string;
  child_transactions?: string[];
  [key: string]: any;
}

interface V3Trace {
  transactions: Record<string, V3Transaction>;
  [key: string]: any;
}

interface V3TracesResponse {
  traces: V3Trace[];
}

export const TransactionPage: React.FC<TransactionPageProps> = ({ client }) => {
  const { hash } = useParams<{ hash: string }>();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(true);
  const [traces, setTraces] = useState<TransactionInfo[]>([]);
  const [contracts, setContracts] = useState<Map<string, ContractData>>(new Map());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hash) return;

    const fetchTrace = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = (await client.getTraces(hash)) as V3TracesResponse;
        
        if (data.traces && data.traces.length > 0) {
          const trace = data.traces[0];
          const transactionsMap = trace.transactions;
          
          // Helper to find parent LT
          const findParentLt = (targetLt: string): string | null => {
            for (const tx of Object.values(transactionsMap)) {
              if (tx.child_transactions?.includes(targetLt)) {
                return tx.lt;
              }
            }
            return null;
          };

          const backendTransactions: BackendTransaction[] = Object.values(transactionsMap).map(tx => ({
            lt: tx.lt,
            raw_transaction: tx.raw_transaction || "",
            parent_transaction: findParentLt(tx.lt), 
            child_transactions: tx.child_transactions || [],
            shard_account_before: "",
            shard_account: "",
            vm_log_diff: "",
            executor_logs: "",
          }));

          const processed = processTransactions(backendTransactions);
          setTraces(processed);
          
          const contractsMap = new Map<string, ContractData>();
          const addresses = new Set<string>();
          processed.forEach(t => {
            if (t.address) addresses.add(t.address.toString());
          });
          
          let nextLetterCode = 65;
          Array.from(addresses).sort().forEach(addr => {
            contractsMap.set(addr, {
              displayName: fmt.formatAddress(addr),
              address: Address.parse(addr),
              letter: String.fromCharCode(nextLetterCode++)
            });
          });
          setContracts(contractsMap);
        } else {
          setError("Transaction not found or has no trace yet.");
        }
      } catch (e) {
        console.error("Failed to fetch trace:", e);
        setError(e instanceof Error ? e.message : "Failed to load transaction trace");
      } finally {
        setLoading(false);
      }
    };

    fetchTrace();
  }, [hash, client]);

  if (loading) {
    return (
      <div className={styles.centered}>
        <Loader2 className={styles.spinner} />
        <p>Loading transaction trace...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className={styles.centered}>
        <AlertCircle className={styles.errorIcon} />
        <p className={styles.errorText}>{error}</p>
        <button onClick={() => navigate(-1)} className={styles.backButton}>
          <ArrowLeft size={16} /> Go Back
        </button>
      </div>
    );
  }

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <button onClick={() => navigate(-1)} className={styles.backButton}>
          <ArrowLeft size={16} /> Back
        </button>
        <h1 className={styles.title}>Transaction Trace</h1>
        <div className={styles.hash}>{hash}</div>
      </header>

      <div className={styles.content}>
        <div className={styles.treeSection}>
          <TransactionTree
            transactions={traces}
            contracts={contracts}
            allContracts={[]}
          />
        </div>
      </div>
    </div>
  );
};
