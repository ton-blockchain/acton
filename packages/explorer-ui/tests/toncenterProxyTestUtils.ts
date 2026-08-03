export function installMemoryEdgeCache() {
  const originalCaches = Object.getOwnPropertyDescriptor(globalThis, "caches")
  const responses = new Map<string, Response>()
  let putCount = 0

  Object.defineProperty(globalThis, "caches", {
    configurable: true,
    value: {
      default: {
        match(request: Request) {
          return Promise.resolve(responses.get(request.url)?.clone())
        },
        put(request: Request, response: Response) {
          putCount += 1
          responses.set(request.url, response.clone())
          return Promise.resolve()
        },
      },
    },
  })

  return {
    responses,
    get putCount() {
      return putCount
    },
    restore() {
      if (originalCaches) {
        Object.defineProperty(globalThis, "caches", originalCaches)
      } else {
        Reflect.deleteProperty(globalThis, "caches")
      }
    },
  }
}
