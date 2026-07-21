import {useState} from "react"
import {AbiGetMethods, AbiPanel, type AbiTab} from "@acton/transaction-ui/abi"

import {
  abiViewerGalleryAbi,
  argumentCountMethodsAbi,
  complexMethodsAbi,
  galleryAddressSuggestions,
  runGalleryGetMethod,
  scalarMethodsAbi,
} from "./abiViewerGalleryFixtures"
import styles from "./abiViewerGallery.module.css"
import type {ComponentGallery} from "./types"

function ReadonlyAbiSample() {
  const [activeTab, setActiveTab] = useState<AbiTab>("view")

  return (
    <div className={styles.readonly}>
      <AbiPanel
        abi={abiViewerGalleryAbi}
        activeTab={activeTab}
        onTabChange={setActiveTab}
        heightMode="content"
        showSymbolAnchors
      />
    </div>
  )
}

function ScalarMethodsSample() {
  return (
    <div className={styles.panel}>
      <AbiGetMethods
        abi={scalarMethodsAbi}
        runGetMethod={runGalleryGetMethod}
        addressSuggestions={galleryAddressSuggestions}
      />
    </div>
  )
}

function ComplexMethodsSample() {
  return (
    <>
      <p className={styles.hint}>
        The structured argument uses the same controlled ABI editor as message composition, with
        nested structs, arrays, dictionaries, optional addresses, GRAM values, and Cell payloads.
      </p>
      <div className={styles.panel}>
        <AbiGetMethods
          abi={complexMethodsAbi}
          runGetMethod={runGalleryGetMethod}
          addressSuggestions={galleryAddressSuggestions}
        />
      </div>
    </>
  )
}

function ArgumentCountMethodsSample() {
  return (
    <div className={styles.panel}>
      <AbiGetMethods
        abi={argumentCountMethodsAbi}
        runGetMethod={runGalleryGetMethod}
        addressSuggestions={galleryAddressSuggestions}
      />
    </div>
  )
}

export const abiViewerGallery = {
  id: "abi-viewer",
  title: "ABI Viewer & Methods",
  status: "ready",
  summary:
    "Read-only ABI documentation and runnable get methods with schema-driven argument editors.",
  importStatement: 'import { AbiPanel, AbiGetMethods } from "@acton/transaction-ui/abi"',
  agentSummary:
    "Use AbiPanel for documentation-only ABI rendering. Use AbiGetMethods with an injected runGetMethod transport for account-scoped execution; network clients stay outside transaction-ui.",
  usage: [
    "Use AbiPanel where users need signatures, messages, storage, declarations, and errors.",
    "Use AbiGetMethods for a dedicated interactive surface; it supports scalar and nested ABI values through AbiValueEditor.",
    "Inject the network call through runGetMethod and keep account/network selection in the app.",
  ],
  avoid: [
    "Do not add execution controls to the read-only AbiPanel.",
    "Do not rebuild per-type get-method inputs in a consuming page.",
    "Do not fetch ABI metadata or account state inside transaction-ui.",
  ],
  sections: [
    {
      id: "abi-viewer-readonly",
      title: "Read-only ABI",
      description: "Rendered documentation and raw JSON without execution state.",
      content: <ReadonlyAbiSample />,
    },
    {
      id: "abi-methods-scalars",
      title: "Runnable · scalar arguments",
      description: "Address input with suggestions and a mocked successful get-method response.",
      content: <ScalarMethodsSample />,
    },
    {
      id: "abi-methods-complex",
      title: "Runnable · complex arguments",
      description: "A complete nested ABI form, kept visible and editable in the gallery.",
      content: <ComplexMethodsSample />,
    },
    {
      id: "abi-methods-argument-counts",
      title: "Runnable · multiple arguments",
      description:
        "Methods with two, three, and six arguments to verify dense forms and repeated field layouts.",
      content: <ArgumentCountMethodsSample />,
    },
  ],
} satisfies ComponentGallery
