import type {ContractABI, SymTable} from "@ton/tolk-abi-to-typescript"

import {
  formatAbiTyDeclaration,
  formatDeclarationTolk,
  getAbiTyDeclaration,
  type AbiDeclaration,
} from "./abiFormatting"
import {AbiSection, AbiSymbolAnchor, abiSymbolAnchorId, TolkCode} from "./abiShared"
import styles from "./AbiViewer.module.css"

type AbiMessage = Readonly<
  | ContractABI["incoming_messages"][number]
  | ContractABI["incoming_external"][number]
  | ContractABI["outgoing_messages"][number]
  | ContractABI["emitted_events"][number]
>

export function AbiMessagesSection({
  abi,
  symbols,
  showSymbolAnchors,
}: {
  readonly abi: ContractABI
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}) {
  const groups: readonly {
    readonly title: string
    readonly messages: readonly AbiMessage[]
    readonly empty: string
  }[] = [
    {
      title: "Incoming/internal",
      messages: abi.incoming_messages,
      empty: "No incoming internal messages declared",
    },
    {
      title: "Incoming external",
      messages: abi.incoming_external,
      empty: "No incoming external messages declared",
    },
    {title: "Outgoing", messages: abi.outgoing_messages, empty: "No outgoing messages declared"},
    {title: "Emitted events", messages: abi.emitted_events, empty: "No emitted events declared"},
  ]
  const count = groups.reduce((total, group) => total + group.messages.length, 0)

  return (
    <AbiSection title="Messages" count={count}>
      <div className={styles.messageGrid}>
        {groups.map(group => (
          <section key={group.title} className={styles.messageGroup}>
            <header className={styles.messageGroupHeader}>
              <span>{group.title}</span>
              <span className={styles.count}>{group.messages.length}</span>
            </header>
            {group.messages.length > 0 ? (
              group.messages.map(message => (
                <AbiMessageRow
                  key={`${group.title}:${message.body_ty_idx}`}
                  groupTitle={group.title}
                  message={message}
                  symbols={symbols}
                  showSymbolAnchors={showSymbolAnchors}
                />
              ))
            ) : (
              <div className={styles.emptyInline}>{group.empty}</div>
            )}
          </section>
        ))}
      </div>
    </AbiSection>
  )
}

function AbiMessageRow({
  groupTitle,
  message,
  symbols,
  showSymbolAnchors,
}: {
  readonly groupTitle: string
  readonly message: AbiMessage
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}) {
  const declaration = getAbiTyDeclaration(symbols, message.body_ty_idx)
  const messageName = declaration?.name ?? `type-${message.body_ty_idx}`
  const messageId = abiSymbolAnchorId("message", `${groupTitle}-${messageName}`)

  return (
    <div id={messageId} className={styles.messageRow}>
      <div className={styles.symbolLine}>
        <TolkCode value={formatAbiTyDeclaration(symbols, message.body_ty_idx)} />
        <AbiSymbolAnchor show={showSymbolAnchors} id={messageId} label={`Link to ${messageName}`} />
      </div>
      {declaration?.description && (
        <p className={styles.declarationDescription}>{declaration.description}</p>
      )}
    </div>
  )
}

export function AbiStorageSection({
  storage,
  symbols,
  showSymbolAnchors,
}: {
  readonly storage: ContractABI["storage"]
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}) {
  const rows = [
    {label: "storage", tyIdx: storage.storage_ty_idx},
    {label: "storageAtDeployment", tyIdx: storage.storage_at_deployment_ty_idx},
  ].filter((row): row is {label: string; tyIdx: number} => row.tyIdx !== undefined)
  const showStorageLabels = rows.length > 1

  return (
    <AbiSection title="Storage" count={rows.length}>
      {rows.length > 0 ? (
        <div className={styles.rows}>
          {rows.map(row => {
            const declaration = getAbiTyDeclaration(symbols, row.tyIdx)
            const storageId = abiSymbolAnchorId("storage", row.label)
            const showHeader = showStorageLabels || showSymbolAnchors
            return (
              <div id={storageId} key={row.label} className={styles.row}>
                {showHeader && (
                  <div
                    className={showStorageLabels ? styles.storageHeader : styles.storageAnchorRow}
                  >
                    {showStorageLabels && <span className={styles.storageName}>{row.label}</span>}
                    <AbiSymbolAnchor
                      show={showSymbolAnchors}
                      id={storageId}
                      label={`Link to ${row.label}`}
                    />
                  </div>
                )}
                <TolkCode value={formatAbiTyDeclaration(symbols, row.tyIdx)} />
                {declaration?.description && (
                  <p className={styles.declarationDescription}>{declaration.description}</p>
                )}
              </div>
            )
          })}
        </div>
      ) : (
        <div className={styles.emptyInline}>No storage type indexes declared</div>
      )}
    </AbiSection>
  )
}

export function AbiDeclarationsSection({
  declarations,
  symbols,
  showSymbolAnchors,
}: {
  readonly declarations: readonly AbiDeclaration[]
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}) {
  return (
    <AbiSection title="Declarations" count={declarations.length}>
      {declarations.length > 0 ? (
        <div className={styles.declarationList}>
          {declarations.map(declaration => {
            const declarationId = abiSymbolAnchorId("declaration", declaration.name)
            return (
              <details
                id={declarationId}
                key={`${declaration.kind}:${declaration.name}:${declaration.ty_idx}`}
                className={styles.declaration}
              >
                <summary>
                  <span className={styles.declarationName}>{declaration.name}</span>
                  <sup
                    className={`${styles.declarationKind} ${declarationKindClass(declaration.kind)}`}
                  >
                    {declaration.kind}
                  </sup>
                  <AbiSymbolAnchor
                    show={showSymbolAnchors}
                    id={declarationId}
                    label={`Link to ${declaration.name}`}
                    onClick={event => {
                      event.stopPropagation()
                      const details = event.currentTarget.closest("details")
                      if (details instanceof HTMLDetailsElement) details.open = true
                    }}
                  />
                </summary>
                <div className={styles.declarationBody}>
                  <TolkCode value={formatDeclarationTolk(declaration, symbols)} />
                  {declaration.description && (
                    <p className={styles.declarationDescription}>{declaration.description}</p>
                  )}
                </div>
              </details>
            )
          })}
        </div>
      ) : (
        <div className={styles.emptyInline}>No declarations emitted</div>
      )}
    </AbiSection>
  )
}

export function AbiThrownErrorsSection({
  errors,
  showSymbolAnchors,
}: {
  readonly errors: readonly ContractABI["thrown_errors"][number][]
  readonly showSymbolAnchors: boolean
}) {
  return (
    <AbiSection title="Thrown errors" count={errors.length}>
      {errors.length > 0 ? (
        <div className={styles.rows}>
          {errors.map(error => {
            const errorName = error.name ?? String(error.err_code)
            const errorId = abiSymbolAnchorId("error", errorName, String(error.err_code))
            return (
              <div
                id={errorId}
                key={`${error.err_code}:${error.name ?? error.kind}`}
                className={`${styles.errorRow} ${showSymbolAnchors ? "" : styles.errorRowNoAnchor}`}
              >
                <span className={styles.errorCode}>{error.err_code}</span>
                <span className={styles.errorName} title={errorName}>
                  {errorName}
                </span>
                <span className={styles.muted}>{error.description ?? ""}</span>
                <AbiSymbolAnchor
                  show={showSymbolAnchors}
                  id={errorId}
                  label={`Link to ${errorName}`}
                />
              </div>
            )
          })}
        </div>
      ) : (
        <div className={styles.emptyInline}>No thrown errors declared</div>
      )}
    </AbiSection>
  )
}

function declarationKindClass(kind: AbiDeclaration["kind"]): string {
  switch (kind) {
    case "struct":
      return styles.declarationKindStruct
    case "alias":
      return styles.declarationKindAlias
    case "enum":
      return styles.declarationKindEnum
  }
}
