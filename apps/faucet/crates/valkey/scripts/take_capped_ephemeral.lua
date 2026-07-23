local value = redis.call("GETDEL", KEYS[2])

if value then
    redis.call("ZREM", KEYS[1], KEYS[2])
end

return value
