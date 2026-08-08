export function readVerifiedContractsPage(search: string): number {
  const value = new URLSearchParams(search).get("page")
  if (value === null) {
    return 0
  }

  const page = Number(value)
  return Number.isInteger(page) && page > 0 ? page - 1 : 0
}

export function verifiedContractsPageSearch(search: string, page: number): string {
  const params = new URLSearchParams(search)
  const normalizedPage = Number.isFinite(page) ? Math.max(0, Math.trunc(page)) : 0

  if (normalizedPage === 0) {
    params.delete("page")
  } else {
    params.set("page", String(normalizedPage + 1))
  }

  return params.toString()
}
