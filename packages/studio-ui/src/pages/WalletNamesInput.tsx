import {MultiValueInput} from "@acton/ui"

interface WalletNamesInputProps {
  readonly values: readonly string[]
  readonly walletNames: readonly string[]
  readonly onChange: (values: readonly string[]) => void
}

export function WalletNamesInput({values, walletNames, onChange}: WalletNamesInputProps) {
  return (
    <MultiValueInput
      label="Startup accounts"
      description="Initialize selected project wallets and fund each with 100 GRAM"
      placeholder={walletNames.length > 0 ? "Search wallets" : "No wallets configured"}
      values={values}
      options={walletNames}
      onValuesChange={onChange}
    />
  )
}
