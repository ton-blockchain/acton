//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core
import React from "react"
import {createRoot} from "react-dom/client"

import {WalletApp} from "./WalletApp"
import type {WalletVSCodeAPI} from "./wallet-types"

import "../../main.css"
import "../../index.css"

declare function acquireVsCodeApi(): WalletVSCodeAPI

const vscode = acquireVsCodeApi()

const rootElement = document.querySelector("#root")
if (rootElement) {
  const root = createRoot(rootElement)
  root.render(
    <React.StrictMode>
      <WalletApp vscode={vscode} />
    </React.StrictMode>,
  )
}
