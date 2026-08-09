import process from "node:process"

interface BridgeMessage {
  readonly id: number
  readonly recipient: string
  readonly data: string
}

interface Subscriber {
  readonly clientIds: ReadonlySet<string>
  readonly controller: ReadableStreamDefaultController<Uint8Array>
}

const port = Number(process.env.ACTON_UI_E2E_TONCONNECT_BRIDGE_PORT ?? 14_309)
const encoder = new TextEncoder()
const messages: BridgeMessage[] = []
const subscribers = new Set<Subscriber>()
let nextEventId = 1

const corsHeaders = {
  "Access-Control-Allow-Headers": "content-type",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Origin": "*",
}

const encodeSseMessage = (message: BridgeMessage): Uint8Array =>
  encoder.encode(`id: ${message.id}\ndata: ${message.data}\n\n`)

const deliver = (subscriber: Subscriber, message: BridgeMessage): void => {
  if (!subscriber.clientIds.has(message.recipient)) {
    return
  }

  try {
    subscriber.controller.enqueue(encodeSseMessage(message))
  } catch {
    subscribers.delete(subscriber)
  }
}

const createEventStream = (url: URL, signal: AbortSignal): Response => {
  const clientIds = new Set(
    (url.searchParams.get("client_id") ?? "")
      .split(",")
      .map(clientId => clientId.trim())
      .filter(Boolean),
  )
  const lastEventId = Number(url.searchParams.get("last_event_id") ?? 0)
  let subscriber: Subscriber | undefined

  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      subscriber = {clientIds, controller}
      subscribers.add(subscriber)
      controller.enqueue(encoder.encode(": connected\n\n"))

      for (const message of messages) {
        if (message.id > lastEventId) {
          deliver(subscriber, message)
        }
      }

      signal.addEventListener(
        "abort",
        () => {
          if (subscriber) {
            subscribers.delete(subscriber)
          }
        },
        {once: true},
      )
    },
    cancel() {
      if (subscriber) {
        subscribers.delete(subscriber)
      }
    },
  })

  return new Response(stream, {
    headers: {
      ...corsHeaders,
      "Cache-Control": "no-cache",
      "Content-Type": "text/event-stream",
    },
  })
}

const relayMessage = async (request: Request, url: URL): Promise<Response> => {
  const sender = url.searchParams.get("client_id")
  const recipient = url.searchParams.get("to")
  if (!(sender && recipient)) {
    return new Response("client_id and to are required", {status: 400, headers: corsHeaders})
  }

  const data = JSON.stringify({
    from: sender,
    message: await request.text(),
    trace_id: url.searchParams.get("trace_id") ?? undefined,
  })
  const message = {id: nextEventId, recipient, data}
  nextEventId += 1
  messages.push(message)

  for (const subscriber of subscribers) {
    deliver(subscriber, message)
  }

  return new Response(null, {status: 200, headers: corsHeaders})
}

// Bun is the fixture server runtime used by the Playwright webServer command.
// biome-ignore lint/correctness/noUndeclaredVariables: Bun is available at runtime.
Bun.serve({
  hostname: "127.0.0.1",
  port,
  fetch(request) {
    const url = new URL(request.url)
    if (request.method === "OPTIONS") {
      return new Response(null, {status: 204, headers: corsHeaders})
    }
    if (url.pathname === "/health") {
      return Response.json({ok: true}, {headers: corsHeaders})
    }
    if (url.pathname === "/bridge/events" && request.method === "GET") {
      return createEventStream(url, request.signal)
    }
    if (url.pathname === "/bridge/message" && request.method === "POST") {
      return relayMessage(request, url)
    }

    return new Response("Not found", {status: 404, headers: corsHeaders})
  },
})
