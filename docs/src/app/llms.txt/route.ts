import {llms} from "fumadocs-core/source"
import {visibleLlmSource} from "@/lib/source"

export const revalidate = false

export async function GET() {
  const body = llms(visibleLlmSource).index()

  return new Response(body, {
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
    },
  })
}
