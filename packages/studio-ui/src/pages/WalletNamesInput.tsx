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
      description="Wallets from the Acton project to create when the environment starts"
      placeholder={walletNames.length > 0 ? "Search wallets" : "No wallets configured"}
      values={values}
      options={walletNames}
      onValuesChange={onChange}
    />
  )
}
