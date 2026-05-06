import {visibleSource} from "@/lib/source"
import {createFromSource} from "fumadocs-core/search/server"

export const revalidate = false
export const {staticGET: GET} = createFromSource(visibleSource)
