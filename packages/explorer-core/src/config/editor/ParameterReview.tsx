import type {Cell} from "@ton/core"

import {parseConfigParameter, type NetworkConfigParameter} from "../../api/config"
import {ConfigParameterValue} from "../../pages/ConfigPage"

interface ParameterReviewProps {
  readonly index: number
  readonly before?: NetworkConfigParameter
  readonly after: Cell
}

/** Uses the config page's value renderer for both sides of an editable comparison. */
export function ParameterReview({index, before, after}: ParameterReviewProps) {
  return (
    <ConfigParameterValue parameter={parseConfigParameter(index, after)} comparison={{before}} />
  )
}
