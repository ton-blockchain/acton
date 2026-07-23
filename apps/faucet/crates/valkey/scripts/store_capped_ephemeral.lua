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
redis.call("EXPIRE", KEYS[1], ttl_seconds)

return 1
