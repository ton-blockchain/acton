import { Card, CardContent, CardHeader } from "@acton/shared-ui";
import { Check, Copy, Edit2, X } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";

import type { FullAccountState } from "../api/types";
import { useAddressBook, useAddressName } from "../hooks/useAddressBook";

import styles from "./AccountInfo.module.css";
import { formatAddress, formatNano } from "./utils";

interface AccountInfoProps {
  readonly address: string;
  readonly state: FullAccountState;
}

export const AccountInfo: React.FC<AccountInfoProps> = ({ address, state }) => {
  const [isEditing, setIsEditing] = useState(false);
  const [customName, setCustomName] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [loading, setLoading] = useState(false);
  const { setAddressName } = useAddressBook();
  const resolvedName = useAddressName(address);

  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (copied) {
      const timer = setTimeout(() => setCopied(false), 2000);
      return () => clearTimeout(timer);
    }
  }, [copied]);

  useEffect(() => {
    setCustomName(resolvedName);
  }, [resolvedName]);

  const handleStartEdit = () => {
    setEditValue(customName || "");
    setIsEditing(true);
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      await setAddressName(address, editValue || "");
      setCustomName(editValue || null);
      setIsEditing(false);
    } catch (error) {
      console.error("Failed to save name:", error);
    } finally {
      setLoading(false);
    }
  };

  const tonBalance = formatNano(state.balance);
  const usdRate = 1.33; // Mock rate for UI matching
  const usdBalance = ((Number(state.balance) / 1e9) * usdRate).toLocaleString(
    undefined,
    {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    },
  );

  const copyToClipboard = () => {
    void navigator.clipboard.writeText(address);
    setCopied(true);
  };

  return (
    <Card className={styles.card}>
      <CardHeader>
        <div className={styles.addressTitle}>Address</div>
        <div className={styles.addressHeader}>
          {isEditing ? (
            <div className={styles.editContainer}>
              <input
                type="text"
                className={styles.editInput}
                value={editValue}
                onChange={(e) => setEditValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    void handleSave();
                  } else if (e.key === "Escape") {
                    setIsEditing(false);
                  }
                }}
                placeholder="Enter custom name"
              />
              <button
                type="button"
                className={styles.iconButton}
                onClick={handleSave}
                disabled={loading}
              >
                <Check size={18} className={styles.saveIcon} />
              </button>
              <button
                type="button"
                className={styles.iconButton}
                onClick={() => setIsEditing(false)}
              >
                <X size={18} className={styles.cancelIcon} />
              </button>
            </div>
          ) : (
            <div className={styles.addressRow}>
              <div className={styles.addressValue}>
                {customName ? (
                  <span className={styles.customName}>
                    {customName}{" "}
                    <span className={styles.realAddress}>
                      ({formatAddress(address, true)})
                    </span>
                  </span>
                ) : (
                  formatAddress(address, false)
                )}
              </div>
              <button
                type="button"
                className={styles.iconButton}
                onClick={handleStartEdit}
              >
                <Edit2 size={16} />
              </button>
              <button
                type="button"
                className={styles.iconButton}
                onClick={copyToClipboard}
              >
                {copied ? (
                  <Check size={16} className={styles.saveIcon} />
                ) : (
                  <Copy size={16} />
                )}
              </button>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className={styles.grid}>
        <div className={styles.section}>
          <div className={styles.label}>Balance</div>
          <div className={styles.value}>
            {tonBalance} TON{" "}
            <span className={styles.subValue}>≈ $ {usdBalance}</span>
          </div>
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Assets</div>
          <div className={styles.assetRow}>
            <div className={styles.assetIcon}></div>
            <div className={styles.value}>
              0.00 USD₮ <span className={styles.subValue}>and more</span>
            </div>
          </div>
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Details</div>
          <div className={styles.detailsGrid}>
            <span
              className={`${styles.status} ${state.state === "active" ? "" : styles.statusUninitialized}`}
            >
              {state.state}
            </span>
            <span className={styles.tag}>unknown</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
};
