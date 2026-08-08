import {useContext} from "react"

import {ExplorerRoutesContext} from "./explorerRoutesContext"

export const useExplorerRoutePaths = () => useContext(ExplorerRoutesContext)
