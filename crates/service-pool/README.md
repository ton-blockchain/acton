# Service pool

Available since trunk.

`Pool<E>` schedules replayable operations across interchangeable endpoints.
The crate has no transport or blockchain dependencies. P2P and LiteServer
adapters own connections, response validation, and error classification.

Implement `Endpoint` with a stable ID and address, add endpoints with `upsert`,
then call `execute` with a request class and an async operation. Use
`execute_where` when only some endpoints can satisfy a request. Discovery can
update the pool while operations are running.

Use `execute_with_probes` to measure other endpoints in background tasks. Its
callback must own its inputs so measurements can outlive the foreground request.

An operation must be read-only or idempotent: retries and speculative attempts
may reach the server even after the caller has stopped waiting. Return `Ok`
only after validating the response. Dropping an attempt must release transport
resources. The pool releases admission independently of transport cleanup.

## Selection and limits

Available since trunk.

Each endpoint has an exponentially weighted mean latency per request class.
Each successful sample contributes 20% to the mean after the first response.
Cancelled attempts contribute no samples. Their elapsed time provides a separate
lower bound for selection until the endpoint completes another response.
Selection multiplies the greater of these estimates by active attempts plus one.
When an adapter sets `min_request_interval`, selection also includes the time
until that endpoint can accept another dispatch. This spreads sequential traffic
across fast endpoints and permits waiting briefly instead of choosing a slow one.
Pacing is shared across classes, clones, retries, and probes. It is disabled by default.
Measured endpoints take priority over unmeasured ones. Foreground requests and speculative
backups use this ranking. For a new request class, measurements from other classes
act as startup hints, ahead of endpoints that have never responded successfully.
Without any measurements, selection rotates endpoints.

Background probes replay successful operations on under-sampled endpoints.
They finish independently of foreground winners and discard their results after
validation and measurement. Recently measured or busy endpoints are skipped.
Each class permits one discovery probe and one recovery probe. Discovery visits
under-sampled endpoints; recovery rechecks previously responsive endpoints in
latency order, including hints from other request classes. It refreshes both
cancellation bounds and successful-response means after temporary slowdowns.
These run independently, so a discovery timeout cannot block recovery in that class.
The default allows four probes in total, spaced at least one second apart for
each class and kind. Probes share admission limits and leave one slot for
foreground work. Dropping the last pool clone cancels outstanding probes.

By default, a slow operation adds a second attempt after 100 ms. The pool tries
at most 16 distinct endpoints per operation, with two attempts in flight at
once. Clones share a total limit of 16 active attempts and four per endpoint.
`Options` lets adapters change these limits and deadlines.

| Outcome | Effect |
| --- | --- |
| Valid response | Return the response, update latency, cancel losing attempts |
| Unavailable data | Try another endpoint; do not penalize its health |
| Transport failure | Retry elsewhere; suspend the endpoint for 2–64 seconds |
| Invalid response | Retry elsewhere; suspend the endpoint for 60 seconds |
| Fatal local or caller error | Stop the operation without penalizing the endpoint |
| Cancelled attempt | Release admission; record a lower bound without changing the successful-response mean |

The request deadline includes admission waits and retries. Snapshot writes
occur after the operation and are outside that deadline.

## Saved measurements

Available since trunk.

Call `persist_to` before cloning the pool to enable atomic JSON snapshots.
The caller supplies a namespace that separates protocols and networks.
A snapshot with a different namespace or version is ignored. Restored entries
remain ineligible until discovery adds their endpoints.

Completed operations trigger a snapshot at most once per ten seconds. Call
`flush` before a controlled shutdown. Abrupt termination can lose recent
measurements. Active attempts, connections, and suspensions are never restored.
Saved latency estimates remain startup hints and background probes refresh them.
An address change clears the endpoint's measurements.

## Calibration

Available since trunk.

Use `measure_all` on a dedicated pool for an explicit calibration pass. It visits
every registered endpoint with bounded concurrency, regardless of rank. Each
endpoint receives one warm-up operation followed by the requested sample count.
The arithmetic mean excludes warm-up and admission waits. A failed operation
stops that endpoint's measurement; incomplete measurements receive no ranking.

Adapters can export `snapshot()` with their public connection descriptors.
Consumers can import it with `restore_snapshot` before normal operation. The
pool checks the namespace and schema; adapters validate connection descriptors.
Calibration is never started automatically by ordinary requests.
