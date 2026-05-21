declare module "bun:test" {
  type TestCallback = () => void | Promise<void>

  type Matchers<T> = {
    toBe(expected: T): void
  }

  export function afterAll(callback: TestCallback): void
  export function beforeAll(callback: TestCallback, timeout?: number): void
  export function expect<T>(value: T): Matchers<T>
  export function test(name: string, callback: TestCallback, timeout?: number): void
}
