import WorldMap, {regions, type DataItem, type ISOCode} from "react-svg-worldmap"

import type {NodeView} from "../types"
import styles from "./NetworkMap.module.css"

interface NetworkMapProps {
  readonly nodes: readonly NodeView[]
}

interface CountryGroup {
  readonly code: string
  readonly country: string
  readonly nodes: readonly NodeView[]
}

const SUPPORTED_COUNTRIES = new Set(regions.map(region => region.code.toLocaleUpperCase()))

/** Shows coarse node distribution without exposing IP addresses to a map provider */
function NetworkMap({nodes}: NetworkMapProps) {
  const countries = groupByCountry(nodes)
  const countriesByCode = new Map(countries.map(country => [country.code, country]))
  const mapData: DataItem[] = countries
    .filter(country => SUPPORTED_COUNTRIES.has(country.code))
    .map(country => ({
      country: country.code.toLocaleLowerCase() as ISOCode,
      value: country.nodes.length,
    }))
  const privateNodes = nodes.filter(node => node.location.kind === "private")
  const unavailableNodes = nodes.filter(node => node.location.kind === "unavailable")

  return (
    <section className={styles.container} aria-label="Node locations by public IP">
      <div className={styles.mapPanel}>
        <div className={styles.map}>
          <WorldMap
            data={mapData}
            size="responsive"
            frame={false}
            backgroundColor="transparent"
            borderColor="var(--acton-color-border)"
            tooltipBgColor="var(--acton-color-surface-raised)"
            tooltipTextColor="var(--acton-color-text)"
            regionClassName={styles.region}
            styleFunction={({countryValue}) => ({
              fill: countryValue ? "var(--acton-color-accent)" : "var(--acton-color-surface-hover)",
              fillOpacity: countryValue ? 0.82 : 0.48,
              stroke: "var(--acton-color-border-strong)",
              strokeWidth: countryValue ? 0.72 : 0.45,
              outline: "none",
            })}
            tooltipTextFunction={({countryCode, countryName}) => {
              const group = countriesByCode.get(countryCode.toLocaleUpperCase())
              if (!group) return countryName

              const names = group.nodes.map(node => node.name).join(", ")
              return `${group.country} — ${names}`
            }}
          />
        </div>

        <div className={styles.attribution}>
          <span>Approximate country-level placement</span>
          <a href="https://db-ip.com" target="_blank" rel="noreferrer">
            IP geolocation by DB-IP
          </a>
        </div>
      </div>

      <div className={styles.summary} aria-label="Node location summary">
        {countries.length === 0 && privateNodes.length === 0 && unavailableNodes.length === 0 ? (
          <p className={styles.empty}>No nodes to locate</p>
        ) : (
          <ul className={styles.locationList}>
            {countries.map(country => (
              <li key={country.code} className={styles.locationRow}>
                <span className={styles.locationText}>
                  <strong>{country.country}</strong>
                  <span>{country.nodes.map(node => node.name).join(", ")}</span>
                </span>
                <span className={styles.locationCount}>
                  {country.nodes.length.toLocaleString()}
                </span>
              </li>
            ))}
            {privateNodes.length > 0 ? (
              <li className={styles.locationRow}>
                <span className={styles.locationText}>
                  <strong>Private network</strong>
                  <span>{privateNodes.map(node => node.name).join(", ")}</span>
                </span>
                <span className={styles.locationCount}>{privateNodes.length.toLocaleString()}</span>
              </li>
            ) : null}
            {unavailableNodes.length > 0 ? (
              <li className={styles.locationRow}>
                <span className={styles.locationText}>
                  <strong>Location unavailable</strong>
                  <span>{unavailableNodes.map(node => node.name).join(", ")}</span>
                </span>
                <span className={styles.locationCount}>
                  {unavailableNodes.length.toLocaleString()}
                </span>
              </li>
            ) : null}
          </ul>
        )}
      </div>
    </section>
  )
}

export default NetworkMap

function groupByCountry(nodes: readonly NodeView[]): readonly CountryGroup[] {
  const groups = new Map<string, {country: string; nodes: NodeView[]}>()

  for (const node of nodes) {
    if (node.location.kind !== "country") continue

    const code = node.location.country_code.toLocaleUpperCase()
    const group = groups.get(code)
    if (group) {
      group.nodes.push(node)
    } else {
      groups.set(code, {country: node.location.country, nodes: [node]})
    }
  }

  return [...groups.entries()]
    .map(([code, group]) => ({code, ...group}))
    .sort((left, right) => left.country.localeCompare(right.country))
}
