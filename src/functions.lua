#!lua name=multilock

local function acquire_lock(keys, args)
    local resources = args
    local lock_id = args[1]
    local expiration = tonumber(args[2])
    
    -- Check for conflicts
    for i = 3, #resources do
        local lock_key = "lock:" .. resources[i]
        local existing_lock = redis.call("GET", lock_key)
        if existing_lock then
            local lock_info = cjson.decode(existing_lock)
            for _, locked_resource in ipairs(lock_info.resources) do
                for j = 3, #resources do
                    if locked_resource == resources[j] then
                        return nil  -- Conflict found
                    end
                end
            end
        end
    end
    
    -- Acquire locks
    local lock_info = cjson.encode({holder = lock_id, resources = {table.unpack(resources, 3)}})
    for i = 3, #resources do
        local lock_key = "lock:" .. resources[i]
        redis.call("SET", lock_key, lock_info, "EX", expiration)
    end
    
    return lock_id
end

local function release_lock(keys, args)
    local lock_id = args[1]
    local cursor = "0"
    local keys_to_delete = {}
    
    repeat
        local result = redis.call("SCAN", cursor, "MATCH", "lock:*")
        cursor = result[1]
        local keys = result[2]
        
        for _, key in ipairs(keys) do
            local lock_data = redis.call("GET", key)
            if lock_data then
                local lock_info = cjson.decode(lock_data)
                if lock_info.holder == lock_id then
                    table.insert(keys_to_delete, key)
                end
            end
        end
    until cursor == "0"
    
    if #keys_to_delete > 0 then
        redis.call("DEL", unpack(keys_to_delete))
    end
    
    return #keys_to_delete
end

redis.register_function('acquire_lock', acquire_lock)
redis.register_function('release_lock', release_lock)