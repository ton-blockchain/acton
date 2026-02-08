import React, {useMemo, useState, useEffect} from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Card,
  CardContent,
} from "@acton/shared-ui";
import { Transaction, FullAccountState } from "../types";
import { formatNano, formatTimeAgo, formatAddress, fetchAddressName } from "./utils";
import { Address, Cell } from "@ton/core";
import { ArrowDownLeft, ArrowUpRight, RefreshCw, Calendar, Filter, MessageSquare, ChevronLeft, ChevronRight, ExternalLink } from "lucide-react";
import styles from "./TransactionList.module.css";
import { ContractCode } from "./ContractCode";
import { useNavigate } from "react-router-dom";

interface TransactionListProps {
  transactions: Transaction[];
  accountState: FullAccountState;
  ownerAddress: string;
  onAddressClick?: (addr: string) => void;
}

const ITEMS_PER_PAGE = 10;

export const TransactionList: React.FC<TransactionListProps> = ({
  transactions,
  accountState,
  ownerAddress,
  onAddressClick
}) => {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"history" | "contract">("history");
  const [currentPage, setCurrentPage] = useState(1);
  const [hoveredAddress, setHoveredAddress] = useState<string | null>(null);
  const [addressNames, setAddressNames] = useState<Record<string, string>>({});

  const browsedAddr = useMemo(() => {
    try {
      return Address.parse(ownerAddress);
    } catch {
      return null;
    }
  }, [ownerAddress]);

  const totalPages = Math.ceil(transactions.length / ITEMS_PER_PAGE);
  const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
  const paginatedTransactions = transactions.slice(startIndex, startIndex + ITEMS_PER_PAGE);

  useEffect(() => {
    const addressesToFetch = new Set<string>();
    paginatedTransactions.forEach(tx => {
      const inAddr = tx.in_msg.source?.account_address;
      const outAddr = tx.out_msgs.find(m => m.destination?.account_address)?.destination?.account_address;
      if (inAddr) addressesToFetch.add(inAddr);
      if (outAddr) addressesToFetch.add(outAddr);
    });

    Array.from(addressesToFetch).forEach(addr => {
      fetchAddressName(addr).then(name => {
        if (name) {
          setAddressNames(prev => {
            if (prev[addr] === name) return prev;
            return { ...prev, [addr]: name };
          });
        }
      });
    });
  }, [paginatedTransactions]);

  return (
    <Card className={styles.tableCard}>
      <div className={styles.tabs}>
        <div
          className={`${styles.tab} ${activeTab === "history" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("history")}
        >
          <RefreshCw size={14} /> History
        </div>
        <div className={styles.tab}>
          <div style={{ width: 14, height: 14, borderRadius: "50%", border: "1px solid currentColor" }} /> Tokens
        </div>
        <div className={styles.tab}>
          <div style={{ width: 14, height: 14, border: "1px solid currentColor" }} /> NFTs
        </div>
        <div
          className={`${styles.tab} ${activeTab === "contract" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("contract")}
        >
          <MessageSquare size={14} /> Contract
        </div>
        <div style={{ flex: 1 }} />
        {activeTab === "history" && (
          <>
            <div className={styles.tab}>
              <Calendar size={14} />
            </div>
            <div className={styles.tab}>
              <Filter size={14} /> Filters
            </div>
          </>
        )}
      </div>

      {activeTab === "history" ? (
        <CardContent style={{ padding: 0 }}>
          <Table>
            <TableHeader>
              <TableRow style={{ borderBottom: "1px solid var(--tonscan-border)" }}>
                <TableHead className={styles.tableHeader}>Time</TableHead>
                <TableHead className={styles.tableHeader}>Action</TableHead>
                <TableHead className={styles.tableHeader}>Address</TableHead>
                <TableHead className={`${styles.tableHeader} ${styles.valueContainer}`}>Value</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {paginatedTransactions.map((tx) => {
                const inMsg = tx.in_msg;

                let isIncoming = false;
                let debugReason = "";

                try {
                  const inMsgSrc = inMsg.source?.account_address ? Address.parse(inMsg.source.account_address) : null;

                  // Logic relative to browsedAddr:
                  // 1. If there's a source and it's NOT us, it's incoming.
                  // 2. If there's no source (external) or source is US, it's outgoing.
                  if (inMsgSrc && browsedAddr && !inMsgSrc.equals(browsedAddr)) {
                    isIncoming = true;
                    debugReason = "in_msg source is not browsed address";
                  } else {
                    isIncoming = false;
                    debugReason = inMsgSrc ? "in_msg source is browsed address" : "external message";
                  }
                } catch (e) {
                  isIncoming = false;
                  debugReason = "error parsing addresses";
                }

                const inValue = BigInt(tx.in_msg.value || "0");
                const outValue = tx.out_msgs.reduce((acc, msg) => acc + BigInt(msg.value || "0"), BigInt(0));

                // For "Sent", we show the value we sent out.
                // For "Received", we show the value that came in.
                const displayValue = isIncoming ? inValue : outValue;
                const valueStr = formatNano(displayValue.toString());

                // Determine counterparty address:
                // If incoming: show the sender (source of in_msg)
                // If outgoing: show the receiver (destination of first out_msg)
                const address = isIncoming 
                  ? (tx.in_msg.source?.account_address || "") 
                  : (tx.out_msgs.find(m => m.destination?.account_address)?.destination?.account_address || "");
                
                const displayAddress = addressNames[address] || (address ? formatAddress(address) : (isIncoming ? "External" : "Contract"));

                const opcode = isIncoming ? tx.in_msg.opcode : tx.out_msgs.find(m => m.opcode)?.opcode;
                const displayOpcode = opcode ? opcode : null;

                const isAddressHovered = hoveredAddress && address && (() => {
                  try {
                    return Address.parse(address).equals(Address.parse(hoveredAddress));
                  } catch {
                    return address === hoveredAddress;
                  }
                })();

                return (
                  <TableRow 
                    key={tx.hash} 
                    className={`${styles.row} ${styles.clickableRow}`}
                    onClick={() => navigate(`/tx/${tx.hash}`)}
                  >
                    <TableCell className={styles.time}>
                      {formatTimeAgo(tx.utime)}
                    </TableCell>
                    <TableCell>
                      <div className={styles.action}>
                        {isIncoming ? (
                          <ArrowDownLeft className={`${styles.actionIcon} ${styles.statusSuccess}`} />
                        ) : (
                          <ArrowUpRight className={`${styles.actionIcon} ${styles.statusFailed}`} />
                        )}
                        <span>{isIncoming ? "Received TON" : "Sent TON"}</span>
                      </div>
                    </TableCell>
                  <TableCell>
                    <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                      <div 
                        className={`${styles.address} ${isAddressHovered ? styles.addressHighlighted : ""}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          if (address) onAddressClick?.(address);
                        }}
                        onMouseEnter={() => address && setHoveredAddress(address)}
                        onMouseLeave={() => setHoveredAddress(null)}
                      >
                        {displayAddress}
                      </div>
                      {displayOpcode && (
                        <span className={styles.opcode}>{displayOpcode}</span>
                      )}
                    </div>
                  </TableCell>
                    <TableCell className={styles.valueContainer}>
                      <div className={isIncoming ? styles.valuePositive : styles.valueNegative}>
                        {isIncoming ? "+" : "-"} {parseFloat(valueStr).toLocaleString()} TON
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>

          {totalPages > 1 && (
            <div className={styles.pagination}>
              <button
                className={styles.paginationButton}
                onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                disabled={currentPage === 1}
              >
                <ChevronLeft size={16} />
              </button>
              <span className={styles.paginationInfo}>
                Page {currentPage} of {totalPages}
              </span>
              <button
                className={styles.paginationButton}
                onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                disabled={currentPage === totalPages}
              >
                <ChevronRight size={16} />
              </button>
            </div>
          )}
        </CardContent>
      ) : (
        <ContractCode codeBoc={accountState.code} />
      )}
    </Card>
  );
};
