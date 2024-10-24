#!lua name=multilock

local function check_conflicts(resources)
    for i = 1, #resources do
        local lock_key = "lock:" .. resources[i]
        local existing_lock = redis.call("GET", lock_key)
        if existing_lock then
            local lock_info = cjson.decode(existing_lock)
            for _, locked_resource in ipairs(lock_info.resources) do
                for j = 1, #resources do
                    if locked_resource == resources[j] then
                        return true  -- Conflict found
                    end
                end
            end
        end
    end
    return false  -- No conflict
end

local function set_locks(lock_id, resources, expiration)
    local lock_info = cjson.encode({holder = lock_id, resources = resources})
    for i = 1, #resources do
        local lock_key = "lock:" .. resources[i]
        redis.call("SET", lock_key, lock_info, "EX", expiration)
    end
end

local function acquire_lock(keys, args)
    local lock_id = args[1]
    local expiration = tonumber(args[2])
    local resources = {}
    for i = 3, #args do
        table.insert(resources, args[i])
    end
    
    if #resources == 0 then
        return redis.error_reply("No resources specified")
    end
    
    if check_conflicts(resources) then
        return nil  -- Conflict found
    end
    
    set_locks(lock_id, resources, expiration)
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

-- Register functions
redis.register_function('acquire_lock', acquire_lock)
redis.register_function('release_lock', release_lock)