import {baseUrl} from "@/lib/metadata"

export const dynamic = "force-static"
export const revalidate = false

export function GET() {
  return Response.json({
    url: baseUrl,
    name: "Acton",
    iconUrl: `${baseUrl}/logo.png`,
  })
}
