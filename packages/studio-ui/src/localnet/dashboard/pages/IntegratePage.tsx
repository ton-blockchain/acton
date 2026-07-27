import type {FC} from "react"

import type {TonClient} from "../../explorer/api/client"
import {EnvironmentConnect} from "../components/EnvironmentConnect"

import styles from "./IntegratePage.module.css"

interface IntegratePageProps {
  readonly client: TonClient
}

export const IntegratePage: FC<IntegratePageProps> = ({client}) => (
  <div className={styles.page}>
    <EnvironmentConnect client={client} />
  </div>
)
