import {Upload} from "lucide-react"
import type {ChangeEvent, FC} from "react"
import {useState} from "react"

import styles from "./JsonUploadField.module.css"

interface JsonUploadFieldProps {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
}

export const JsonUploadField: FC<JsonUploadFieldProps> = ({label, value, onChange}) => {
  const [fileName, setFileName] = useState("")

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) {
      return
    }
    setFileName(file.name)
    onChange(await file.text())
    event.target.value = ""
  }

  return (
    <label className={styles.fieldLabel}>
      {label}
      <span className={styles.fileControl}>
        <span className={styles.fileButton}>
          <Upload size={15} />
          Choose file
        </span>
        <span className={styles.fileName}>{fileName || "No file chosen"}</span>
        <input
          className={styles.fileInput}
          type="file"
          accept="application/json,.json"
          onChange={event => {
            void handleFileChange(event)
          }}
        />
      </span>
      <textarea
        className={styles.textArea}
        value={value}
        onChange={event => onChange(event.target.value)}
        spellCheck="false"
      />
    </label>
  )
}
