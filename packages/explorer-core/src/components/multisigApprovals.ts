import type {V3Action, V3MultisigOrder} from "../api/types"

export function collectMultisigApprovalTimes(
  order: V3MultisigOrder,
  actions: readonly V3Action[],
): ReadonlyMap<string, number> {
  const orderKey = multisigAddressKey(order.address)
  const approvedSignerKeys = new Set(
    order.signers
      .filter((_, index) => isMultisigSignerApproved(order, index))
      .map(multisigAddressKey),
  )
  const createTimes = new Map<string, number>()
  const explicitApprovalTimes = new Map<string, number>()

  for (const action of actions) {
    if (action.success !== true || !isValidTimestamp(action.end_utime)) {
      continue
    }

    if (action.type === "multisig_approve") {
      const {destination, exit_code: exitCode, source} = action.details
      if (
        !source ||
        !destination ||
        multisigAddressKey(destination) !== orderKey ||
        (exitCode !== null && exitCode !== 0)
      ) {
        continue
      }

      const sourceKey = multisigAddressKey(source)
      if (approvedSignerKeys.has(sourceKey)) {
        setEarliestTime(explicitApprovalTimes, sourceKey, action.end_utime)
      }
      continue
    }

    if (action.type !== "multisig_create_order") {
      continue
    }

    const {
      creator_index: creatorIndex,
      destination_order: destinationOrder,
      is_created_by_signer: isCreatedBySigner,
      source,
    } = action.details
    if (
      !source ||
      !destinationOrder ||
      isCreatedBySigner !== true ||
      multisigAddressKey(destinationOrder) !== orderKey ||
      !Number.isInteger(creatorIndex) ||
      creatorIndex === null ||
      creatorIndex < 0 ||
      creatorIndex >= order.signers.length
    ) {
      continue
    }

    const sourceKey = multisigAddressKey(source)
    if (
      multisigAddressKey(order.signers[creatorIndex]) === sourceKey &&
      approvedSignerKeys.has(sourceKey)
    ) {
      setEarliestTime(createTimes, sourceKey, action.end_utime)
    }
  }

  // Some orders are deployed with the creator's approval bit already set even when
  // the parsed create action does not expose it. A later explicit approval is more
  // precise and therefore replaces the creation-time fallback.
  return new Map([...createTimes, ...explicitApprovalTimes])
}

export function isMultisigSignerApproved(order: V3MultisigOrder, index: number): boolean {
  if (order.approvals_mask === null || index < 0) {
    return false
  }
  try {
    return (BigInt(order.approvals_mask) & (1n << BigInt(index))) !== 0n
  } catch {
    return false
  }
}

export function multisigAddressKey(address: string): string {
  return address.trim().toLowerCase()
}

export function compareMultisigApprovalTimes(
  left: number | undefined,
  right: number | undefined,
): number {
  if (left === undefined) {
    return right === undefined ? 0 : 1
  }
  if (right === undefined) {
    return -1
  }
  return left - right
}

function setEarliestTime(times: Map<string, number>, address: string, timestamp: number): void {
  const current = times.get(address)
  if (current === undefined || timestamp < current) {
    times.set(address, timestamp)
  }
}

function isValidTimestamp(timestamp: number): boolean {
  return Number.isFinite(timestamp) && timestamp > 0
}
