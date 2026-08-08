local window_key = KEYS[1]
local sequence_key = KEYS[2]

local window_ms = tonumber(ARGV[1]) * 1000
local ttl_seconds = tonumber(ARGV[2])

local now = redis.call('TIME')
local now_ms = (tonumber(now[1]) * 1000) + math.floor(tonumber(now[2]) / 1000)
local min_score = now_ms - window_ms

redis.call('ZREMRANGEBYSCORE', window_key, '-inf', min_score)

local sequence = redis.call('INCR', sequence_key)
local member = tostring(now_ms) .. ':' .. tostring(sequence)

redis.call('ZADD', window_key, now_ms, member)
redis.call('EXPIRE', window_key, ttl_seconds)
redis.call('EXPIRE', sequence_key, ttl_seconds)

return redis.call('ZCARD', window_key)
