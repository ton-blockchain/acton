import {Check, KeyRound, Settings, Star} from "lucide-react"
import type {FC} from "react"
import {useLocation, useNavigate} from "react-router"
import {Tooltip} from "@acton/ui"

import {supports} from "../../environmentCapabilities"
import {useLocalnetRuntime} from "../LocalnetRuntimeProvider"
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
  const {environment} = useLocalnetRuntime()
  const localPathname = location.pathname.slice(routes.basePath.length) || "/"

  return (
    <>
      {supports(environment, "controlApi") ? (
        <Tooltip
          content={localnetApiToken ? "Environment API token set" : "Set environment API token"}
        >
          <button
            type="button"
            className={`${styles.sidebarUtilityButton} ${
              localnetApiToken ? styles.sidebarUtilityButtonActive : ""
            }`}
            onClick={onOpenAuthTokenOverlay}
            aria-label={
              localnetApiToken ? "Edit environment API token" : "Set environment API token"
            }
          >
            <KeyRound size={18} />
            {localnetApiToken ? (
              <Check size={12} className={styles.utilityStatusIcon} />
            ) : undefined}
          </button>
        </Tooltip>
      ) : undefined}

      {supports(environment, "explorer") ? (
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
      ) : undefined}

      {environment?.lifecycle === "managed" ? (
        <Tooltip content="Environment settings">
          <button
            type="button"
            className={`${styles.sidebarUtilityButton} ${
              localPathname === "/settings" ? styles.sidebarUtilityButtonActive : ""
            }`}
            onClick={() => void navigate(routes.path("/settings"))}
            aria-label="Environment settings"
          >
            <Settings size={18} />
          </button>
        </Tooltip>
      ) : undefined}
    </>
  )
}
