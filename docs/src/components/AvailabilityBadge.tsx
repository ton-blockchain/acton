export function AvailabilityBadge({since}: {since: string}) {
  return (
    <div className="availability-badge not-prose inline-flex items-center gap-2 rounded-md border border-fd-primary/25 bg-fd-primary/10 px-2.5 py-1 text-xs font-medium text-fd-primary">
      <span className="size-1.5 rounded-full bg-fd-primary" />
      <span>Available since: {since}</span>
    </div>
  )
}
