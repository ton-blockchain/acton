import {TokenAmount} from "@acton/ui"

import styles from "./gramAmountGallery.module.css"
import type {ComponentGallery} from "./types"

function TokenAmountSamples() {
  return (
    <div className={styles.grid}>
      {[
        ["Jetton", <TokenAmount key="jetton" decimals={9} symbol="ACT" value="1000000000000000" />],
        ["Six decimals", <TokenAmount key="six" decimals={6} symbol="USD₮" value="123456789" />],
        [
          "Large exact value",
          <TokenAmount
            key="large"
            decimals={18}
            symbol="JET"
            useGrouping
            value="123456789012345678901234567890"
          />,
        ],
        [
          "Signed raw units",
          <TokenAmount
            key="signed"
            decimals={0}
            signDisplay="except-zero"
            symbol="NFT"
            value={42n}
          />,
        ],
      ].map(([label, value]) => (
        <div className={styles.item} key={label as string}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  )
}

export const tokenAmountGallery = {
  id: "token-amount",
  title: "TokenAmount",
  status: "ready",
  summary: "TokenAmount renders exact integer token units with token-specific precision",
  importStatement: 'import {TokenAmount, formatTokenAmount} from "@acton/ui"',
  agentSummary:
    "Use TokenAmount for rendered token balances and formatTokenAmount only where JSX is unavailable",
  usage: [
    "Pass raw integer units and the decimals declared by the token metadata",
    "If the precision is unknown, use zero decimals to keep the value raw",
    "Pass the symbol to include it in the value and identify the amount in the tooltip",
    "Keep the tooltip enabled when the visible value is rounded or abbreviated",
  ],
  avoid: [
    "Do not divide token values through Number or a floating-point power of ten",
    "Do not append a token symbol to a separately formatted decimal string",
    "Do not create local token, Jetton, asset, or supply formatters",
  ],
  sections: [
    {
      id: "token-amount-formats",
      title: "Formats",
      description:
        "Every format keeps the decimal precision and exact raw integer units available in the tooltip",
      content: <TokenAmountSamples />,
    },
  ],
} satisfies ComponentGallery
