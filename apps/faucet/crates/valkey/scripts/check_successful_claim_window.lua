local window_key = KEYS[1]

local limit = tonumber(ARGV[1])
local window_ms = tonumber(ARGV[2]) * 1000
local ttl_seconds = tonumber(ARGV[3])

local now = redis.call('TIME')
local now_ms = (tonumber(now[1]) * 1000) + math.floor(tonumber(now[2]) / 1000)
local min_score = now_ms - window_ms

redis.call('ZREMRANGEBYSCORE', window_key, '-inf', min_score)

local current = redis.call('ZCARD', window_key)

if current >= limit then
    local retry_after_ms = window_ms
    local oldest = redis.call('ZRANGE', window_key, 0, 0, 'WITHSCORES')

    if oldest[2] ~= nil then
        retry_after_ms = tonumber(oldest[2]) + window_ms - now_ms + 1
        if retry_after_ms < 1 then
            retry_after_ms = 1
        end
    end

    redis.call('EXPIRE', window_key, ttl_seconds)

    return {0, current, retry_after_ms}
end

redis.call('EXPIRE', window_key, ttl_seconds)

return {1, current, 0}
