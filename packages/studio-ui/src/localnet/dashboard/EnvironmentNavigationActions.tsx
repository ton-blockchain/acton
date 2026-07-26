import {Check, KeyRound, Star} from "lucide-react"
import type {FC} from "react"
import {useLocation, useNavigate} from "react-router"
import {Tooltip} from "@acton/ui"

import {useLocalnetRoutes} from "../routes"

import styles from "./DashboardPage.module.css"

interface EnvironmentNavigationActionsProps {
  readonly localnetApiToken?: string
  readonly onOpenAuthTokenOverlay: () => void
}

export const EnvironmentNavigationActions: FC<EnvironmentNavigationActionsProps> = ({
  localnetApiToken,
  onOpenAuthTokenOverlay,
}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const routes = useLocalnetRoutes()
  const localPathname = location.pathname.slice(routes.basePath.length) || "/"

  return (
    <>
      <Tooltip content={localnetApiToken ? "Localnet API token set" : "Set localnet API token"}>
        <button
          type="button"
          className={`${styles.sidebarUtilityButton} ${
            localnetApiToken ? styles.sidebarUtilityButtonActive : ""
          }`}
          onClick={onOpenAuthTokenOverlay}
          aria-label={localnetApiToken ? "Edit localnet API token" : "Set localnet API token"}
        >
          <KeyRound size={18} />
          {localnetApiToken ? <Check size={12} className={styles.utilityStatusIcon} /> : undefined}
        </button>
      </Tooltip>

      <Tooltip content="Favorites">
        <button
          type="button"
          className={`${styles.sidebarUtilityButton} ${
            localPathname === "/explorer/favorites" ? styles.sidebarUtilityButtonActive : ""
          }`}
          onClick={() => void navigate(routes.path("/explorer/favorites"))}
          aria-label="Favorites"
        >
          <Star size={18} />
        </button>
      </Tooltip>
    </>
  )
}
