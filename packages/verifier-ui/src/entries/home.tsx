import {createRoot} from "react-dom/client"

import {HomePage} from "../pages/HomePage"
import "../global.css"

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(<HomePage />)
