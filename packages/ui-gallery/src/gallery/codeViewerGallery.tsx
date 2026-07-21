import {CodeViewer, type CodeViewerFile} from "@acton/ui"

import errorsSource from "../../../../src/commands/new/templates/jetton/contracts/errors.tolk?raw"
import feesManagementSource from "../../../../src/commands/new/templates/jetton/contracts/fees-management.tolk?raw"
import jettonUtilsSource from "../../../../src/commands/new/templates/jetton/contracts/jetton-utils.tolk?raw"
import jettonWalletSource from "../../../../src/commands/new/templates/jetton/contracts/JettonWallet.tolk?raw"
import messagesSource from "../../../../src/commands/new/templates/jetton/contracts/messages.tolk?raw"
import shardingSource from "../../../../src/commands/new/templates/jetton/contracts/sharding.tolk?raw"
import storageSource from "../../../../src/commands/new/templates/jetton/contracts/storage.tolk?raw"
import commonScriptSource from "../../../../src/commands/new/templates/jetton/scripts/utils/common.tolk?raw"
import jettonWalletWrapperSource from "../../../../src/commands/new/templates/jetton/wrappers/JettonWallet.gen.tolk?raw"
import styles from "./codeViewerGallery.module.css"
import type {ComponentGallery} from "./types"

const sourceFiles = [
  {
    path: "contracts/JettonWallet.tolk",
    content: jettonWalletSource,
  },
  {
    path: "contracts/errors.tolk",
    content: errorsSource,
  },
  {
    path: "contracts/fees-management.tolk",
    content: feesManagementSource,
  },
  {
    path: "contracts/jetton-utils.tolk",
    content: jettonUtilsSource,
  },
  {
    path: "contracts/messages.tolk",
    content: messagesSource,
  },
  {
    path: "contracts/sharding.tolk",
    content: shardingSource,
  },
  {
    path: "contracts/storage.tolk",
    content: storageSource,
  },
  {
    path: "scripts/utils/common.tolk",
    content: commonScriptSource,
  },
  {
    path: "wrappers/JettonWallet.gen.tolk",
    content: jettonWalletWrapperSource,
  },
] satisfies readonly CodeViewerFile[]

export const codeViewerGallery = {
  id: "code-viewer",
  title: "CodeViewer",
  status: "ready",
  summary:
    "CodeViewer combines a collapsible source tree, active-file selection, line numbers, syntax highlighting, copying, and responsive navigation.",
  importStatement: 'import {CodeViewer} from "@acton/ui"',
  agentSummary:
    "Use CodeViewer for read-only multi-file source bundles. Pass only paths and contents; keep loading, verification, compilation, and domain metadata outside it.",
  usage: [
    "Use for verified contracts, generated source bundles, and other read-only multi-file code.",
    "Pass entrypoint when the primary file should be selected and marked as main.",
    "Use the optional external action for verification or repository navigation.",
    "Use compact inside transaction details and other vertically constrained panels.",
  ],
  avoid: [
    "Do not use for editable source; use CodeEditor instead.",
    "Do not pass verifier ABI objects or fetch data inside the component.",
    "Do not rebuild file-tree, copy, line-number, or responsive behavior in domain UI.",
  ],
  sections: [
    {
      id: "code-viewer-tree",
      title: "Nested Source Tree",
      description:
        "The real Jetton project checks long-file scrolling. Folders expand independently, and row hover changes both label and icon color.",
      content: (
        <CodeViewer
          files={sourceFiles}
          entrypoint="contracts/JettonWallet.tolk"
          externalActionUrl="https://verifier.acton.monster/example"
          externalActionLabel="View verification"
        />
      ),
    },
    {
      id: "code-viewer-compact",
      title: "Compact",
      description: "The same viewer with a shorter source area and no external action.",
      content: (
        <div className={styles.compactFrame}>
          <CodeViewer files={sourceFiles} entrypoint="contracts/messages.tolk" compact />
        </div>
      ),
    },
    {
      id: "code-viewer-empty",
      title: "Empty",
      description: "An explicit placeholder when a source bundle contains no files.",
      content: <CodeViewer files={[]} emptyMessage="No source files available" />,
    },
  ],
} satisfies ComponentGallery
