import React from "react";
import { Card, CardContent, CardHeader, Tooltip } from "@acton/shared-ui";
import { FullAccountState } from "../types";
import { formatNano, formatAddress } from "./utils";
import styles from "./AccountInfo.module.css";
import { Copy, Settings, Bell } from "lucide-react";

interface AccountInfoProps {
  address: string;
  state: FullAccountState;
}

export const AccountInfo: React.FC<AccountInfoProps> = ({ address, state }) => {
  const tonBalance = parseFloat(formatNano(state.balance));
  const usdRate = 1.33; // Mock rate for UI matching
  const usdBalance = (tonBalance * usdRate).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

  const copyToClipboard = () => {
    navigator.clipboard.writeText(address);
  };

  return (
    <Card style={{ backgroundColor: "var(--tonscan-card-bg)", border: "1px solid var(--tonscan-border)" }}>
      <CardHeader>
        <div className={styles.addressTitle}>Address</div>
        <div className={styles.addressHeader}>
          <div className={styles.addressValue}>{formatAddress(address, false)}</div>
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
