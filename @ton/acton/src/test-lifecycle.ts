type TestHook = (callback: () => Promise<void>) => void

type BunTestModule = {
  readonly afterAll?: TestHook
  readonly afterEach?: TestHook
}

const BUN_TEST_MODULE = "bun:test"

export async function registerAfterEach(callback: () => Promise<void>): Promise<boolean> {
  return registerTestHook("afterEach", callback)
}

export async function registerAfterAll(callback: () => Promise<void>): Promise<boolean> {
  return registerTestHook("afterAll", callback)
}

async function registerTestHook(
  hook: "afterAll" | "afterEach",
  callback: () => Promise<void>,
): Promise<boolean> {
  const module = await importOptionalBunTest()
  const register = module?.[hook]
  if (!register) {
    return false
  }

  try {
    register(callback)
    return true
  } catch {
    return false
  }
}

async function importOptionalBunTest(): Promise<BunTestModule | null> {
  try {
    return (await import(BUN_TEST_MODULE)) as BunTestModule
  } catch {
    return null
  }
}
