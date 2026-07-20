type ClassDictionary = Readonly<Record<string, boolean | null | undefined>>
type ClassArray = readonly ClassValue[]

export type ClassValue = string | number | false | null | undefined | ClassArray | ClassDictionary

export function cx(...values: readonly ClassValue[]): string {
  const classes: string[] = []

  const append = (value: ClassValue): void => {
    if (!value) return

    if (typeof value === "string" || typeof value === "number") {
      classes.push(String(value))
      return
    }

    if (Array.isArray(value)) {
      for (const item of value) {
        append(item)
      }
      return
    }

    for (const [className, enabled] of Object.entries(value)) {
      if (enabled) classes.push(className)
    }
  }

  for (const value of values) {
    append(value)
  }

  return classes.join(" ")
}
