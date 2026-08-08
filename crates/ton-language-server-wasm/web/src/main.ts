import {createElement} from "react"
import {createRoot} from "react-dom/client"

import {App} from "./App"
import "./style.css"

const root = document.getElementById("app")

if (!root) {
  throw new Error("TON LS root element is missing")
}

createRoot(root).render(createElement(App))
