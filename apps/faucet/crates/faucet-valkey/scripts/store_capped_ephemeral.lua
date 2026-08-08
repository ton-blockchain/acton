local ttl_seconds = tonumber(ARGV[2])
local max_entries = tonumber(ARGV[3])
local now = redis.call("TIME")
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)

redis.call("ZREMRANGEBYSCORE", KEYS[1], "-inf", now_ms)

if redis.call("ZCARD", KEYS[1]) >= max_entries then
    return 0
end

local stored = redis.call("SET", KEYS[2], ARGV[1], "EX", ttl_seconds, "NX")
if not stored then
    return -1
end

redis.call("ZADD", KEYS[1], now_ms + ttl_seconds * 1000, KEYS[2])

local latest = redis.call("ZREVRANGE", KEYS[1], 0, 0, "WITHSCORES")
if latest[2] then
    -- Keep the index at least as long as its longest-lived value. The extra
    -- second covers the small gap between TIME and SET's expiry timestamp.
    redis.call("PEXPIREAT", KEYS[1], math.ceil(tonumber(latest[2])) + 1000)
end

return 1
