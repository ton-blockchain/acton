import {createAPIPage} from "fumadocs-openapi/ui"
import type {ComponentProps} from "react"
import {openapi} from "@/lib/openapi"

const FumadocsAPIPage = createAPIPage(openapi)

type APIPageProps = ComponentProps<typeof FumadocsAPIPage>

export function APIPage(props: APIPageProps) {
  return <FumadocsAPIPage showDescription {...props} />
}
