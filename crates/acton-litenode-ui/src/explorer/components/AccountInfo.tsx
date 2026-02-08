import React, { useState, useEffect } from "react";
import { Card, CardContent, CardHeader } from "@acton/shared-ui";
import { FullAccountState } from "../types";
import { formatNano, formatAddress, fetchAddressName, updateCachedAddressName, tonClientInstance } from "./utils";
import styles from "./AccountInfo.module.css";
import { Copy, Edit2, Check, X } from "lucide-react";

interface AccountInfoProps {
  address: string;
  state: FullAccountState;
}

export const AccountInfo: React.FC<AccountInfoProps> = ({ address, state }) => {
  const [isEditing, setIsEditing] = useState(false);
  const [customName, setCustomName] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [loading, setLoading] = useState(false);

  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (copied) {
      const timer = setTimeout(() => setCopied(false), 2000);
      return () => clearTimeout(timer);
    }
  }, [copied]);

  useEffect(() => {
    fetchAddressName(address).then(name => {
      setCustomName(name);
    });
  }, [address]);

  const handleStartEdit = () => {
    setEditValue(customName || "");
    setIsEditing(true);
  };

  const handleSave = async () => {
    if (!tonClientInstance) return;
    setLoading(true);
    try {
      await tonClientInstance.setAddressName(address, editValue);
      updateCachedAddressName(address, editValue || null);
      setCustomName(editValue || null);
      setIsEditing(false);
    } catch (e) {
      console.error("Failed to save name:", e);
    } finally {
      setLoading(false);
    }
  };

  const tonBalance = parseFloat(formatNano(state.balance));
  const usdRate = 1.33; // Mock rate for UI matching
  const usdBalance = (tonBalance * usdRate).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

  const copyToClipboard = () => {
    navigator.clipboard.writeText(address);
    setCopied(true);
  };

  return (
    <Card style={{ backgroundColor: "var(--tonscan-card-bg)", border: "1px solid var(--tonscan-border)" }}>
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
                autoFocus
              />
              <button className={styles.iconButton} onClick={handleSave} disabled={loading}>
                <Check size={18} className={styles.saveIcon} />
              </button>
              <button className={styles.iconButton} onClick={() => setIsEditing(false)}>
                <X size={18} className={styles.cancelIcon} />
              </button>
            </div>
          ) : (
            <div className={styles.addressRow}>
              <div className={styles.addressValue}>
                {customName ? (
                  <span className={styles.customName}>{customName} <span className={styles.realAddress}>({formatAddress(address, true, true)})</span></span>
                ) : (
                  formatAddress(address, false, true)
                )}
              </div>
              <button className={styles.iconButton} onClick={handleStartEdit}>
                <Edit2 size={16} />
              </button>
              <button className={styles.iconButton} onClick={copyToClipboard}>
                {copied ? <Check size={16} className={styles.saveIcon} /> : <Copy size={16} />}
              </button>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className={styles.grid}>
        <div className={styles.section}>
          <div className={styles.label}>Balance</div>
          <div className={styles.value}>
            {tonBalance.toLocaleString()} TON <span className={styles.subValue}>≈ $ {usdBalance}</span>
          </div>
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Assets</div>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <div style={{ width: "16px", height: "16px", borderRadius: "50%", backgroundColor: "#22c55e" }}></div>
            <div className={styles.value}>0.00 USD₮ <span className={styles.subValue}>and more</span></div>
          </div>
        </div>
        <div className={styles.section}>
          <div className={styles.label}>Details</div>
          <div className={styles.detailsGrid}>
            <span className={`${styles.status} ${state.state !== "active" ? styles.statusUninitialized : ""}`}>
              {state.state}
            </span>
            <span className={styles.tag}>wallet v4 r2</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
};
