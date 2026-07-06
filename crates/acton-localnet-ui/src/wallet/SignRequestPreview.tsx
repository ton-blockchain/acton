import {KeyRound} from "lucide-react"
import type {FC} from "react"
import type {SignDataRequestEvent} from "@ton/walletkit"

import styles from "../dashboard/pages/WalletsPage.module.css"

import {SignRequestCellPreview} from "./SignRequestCellPreview"

interface SignRequestPreviewProps {
  readonly preview: SignDataRequestEvent["preview"]["data"]
}

export const SignRequestPreview: FC<SignRequestPreviewProps> = ({preview}) => {
  if (preview.type === "cell") {
    return <SignRequestCellPreview preview={preview} />
  }

  return (
    <div className={styles.messageItem}>
      <KeyRound size={16} />
      <div>
        <div className={styles.messageAddress}>{preview.type.toUpperCase()}</div>
        <div className={styles.permissionDescription}>{describeSignPreview(preview)}</div>
      </div>
    </div>
  )
}

function describeSignPreview(preview: SignDataRequestEvent["preview"]["data"]): string {
  switch (preview.type) {
    case "text": {
      return preview.value.content
    }
    case "binary": {
      return `${preview.value.content.length} base64 chars`
    }
    default: {
      return "Unknown sign payload"
    }
  }
}
