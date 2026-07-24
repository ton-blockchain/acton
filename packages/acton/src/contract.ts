import type {Contract, ContractProvider, Transaction} from "@ton/core"

type ProviderMethodName = `send${string}` | `get${string}`

export type ContractHandle<T extends Contract> = {
  readonly [K in keyof T]: K extends ProviderMethodName
    ? T[K] extends (provider: ContractProvider, ...args: infer Args) => infer Result
      ? K extends `send${string}`
        ? (...args: Args) => Promise<Transaction[]>
        : (...args: Args) => Result
      : T[K]
    : T[K]
}

type ProviderMethod<T extends Contract> = (
  this: T,
  provider: ContractProvider,
  ...args: unknown[]
) => unknown

export function createContractHandle<T extends Contract>(
  contract: T,
  provider: ContractProvider,
): ContractHandle<T> {
  const boundMethods = new Map<PropertyKey, unknown>()

  return new Proxy(contract, {
    get(target, property, receiver) {
      const value = Reflect.get(target, property, receiver)
      if (!isProviderMethod(property, value)) {
        return value
      }

      const cached = boundMethods.get(property)
      if (cached) {
        return cached
      }

      const method = value as ProviderMethod<T>
      const bound = (...args: unknown[]): unknown => method.call(target, provider, ...args)
      boundMethods.set(property, bound)
      return bound
    },
  }) as ContractHandle<T>
}

function isProviderMethod(
  property: PropertyKey,
  value: unknown,
): value is ProviderMethod<Contract> {
  return (
    typeof property === "string" &&
    typeof value === "function" &&
    (property.startsWith("send") || property.startsWith("get"))
  )
}
