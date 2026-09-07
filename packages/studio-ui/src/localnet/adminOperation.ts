export const adminOperationPhases: Record<string, string> = {
  preparing: "Preparing operation",
  stopping: "Stopping network",
  backingUp: "Saving recovery snapshots",
  suspending: "Suspending validators",
  building: "Building hardfork",
  installing: "Installing hardfork",
  verifying: "Verifying state on every node",
  resuming: "Checking block production",
  indexing: "Waiting for the indexer",
  restoring: "Restoring previous state",
  completed: "Changes applied",
  failed: "Operation failed",
}
