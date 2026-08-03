import {DEFAULT_PAGE_SIZE, getPaginationState} from "@acton/ui"
import type {PaginationState} from "@acton/ui"
import {useCallback, useEffect, useMemo} from "react"
import {useSearchParams} from "react-router"

interface SearchParamPaginationOptions {
  readonly pageSize?: number
  readonly paramName?: string
  readonly ready?: boolean
}

interface SearchParamPagination<T> extends PaginationState {
  readonly currentItems: readonly T[]
  readonly setCurrentPage: (page: number) => void
}

export function useSearchParamPagination<T>(
  items: readonly T[],
  {
    pageSize = DEFAULT_PAGE_SIZE,
    paramName = "page",
    ready = true,
  }: SearchParamPaginationOptions = {},
): SearchParamPagination<T> {
  const [searchParams, setSearchParams] = useSearchParams()
  const requestedPage = parsePageSearchParam(searchParams.get(paramName))
  const state = getPaginationState(items.length, requestedPage, pageSize)
  const currentItems = useMemo(
    () => items.slice(state.startIndex, state.endIndex),
    [items, state.endIndex, state.startIndex],
  )

  const updatePage = useCallback(
    (page: number, replace: boolean) => {
      const normalizedPage = getPaginationState(items.length, page, pageSize).currentPage
      const nextSearchParams = withPageSearchParam(searchParams, normalizedPage, paramName)

      if (nextSearchParams.toString() !== searchParams.toString()) {
        setSearchParams(nextSearchParams, {replace})
      }
    },
    [items.length, pageSize, paramName, searchParams, setSearchParams],
  )

  useEffect(() => {
    if (!ready) return

    const currentValue = searchParams.get(paramName)
    const canonicalValue = pageSearchParamValue(state.currentPage)
    if (currentValue !== canonicalValue) {
      updatePage(state.currentPage, true)
    }
  }, [paramName, ready, searchParams, state.currentPage, updatePage])

  const setCurrentPage = useCallback(
    (page: number) => {
      updatePage(page, false)
    },
    [updatePage],
  )

  return {
    ...state,
    currentItems,
    setCurrentPage,
  }
}

export function parsePageSearchParam(value: string | null): number {
  if (value === null || !/^\d+$/.test(value)) return 1

  const page = Number(value)
  return Number.isSafeInteger(page) && page >= 1 ? page : 1
}

export function withPageSearchParam(
  searchParams: URLSearchParams,
  page: number,
  paramName = "page",
): URLSearchParams {
  const nextSearchParams = new URLSearchParams(searchParams)
  const value = pageSearchParamValue(page)

  if (value === null) {
    nextSearchParams.delete(paramName)
  } else {
    nextSearchParams.set(paramName, value)
  }

  return nextSearchParams
}

function pageSearchParamValue(page: number): string | null {
  return page > 1 ? String(page) : null
}
