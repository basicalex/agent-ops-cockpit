--- @sync entry
local function get_lock_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	local session = os.getenv("ZELLIJ_SESSION_NAME") or "session"
	local pane = os.getenv("ZELLIJ_PANE_ID") or ""
	session = session:gsub("[^%w%-_]", "_")
	pane = pane:gsub("[^%w%-_]", "_")
	local suffix = session
	if pane ~= "" then
		suffix = session .. "-" .. pane
	end
	return state_root .. "/aoc/yazi-edit-" .. suffix .. ".lock"
end

local function read_lock_title()
	local f = io.open(get_lock_path(), "r")
	if not f then return nil end
	local line = f:read("*l")
	f:close()
	if not line then return nil end
	line = line:gsub("%s+$", "")
	if line == "" then return nil end
	return line
end

local EMPTY_TITLE = " "

local function resolve_title()
	local locked = read_lock_title()
	if locked then return locked end
	return EMPTY_TITLE
end

local function do_rename(title)
	local parts = {}
	local zellij = os.getenv("ZELLIJ")
	local session = os.getenv("ZELLIJ_SESSION_NAME")
	local pane = os.getenv("ZELLIJ_PANE_ID")

	if zellij and zellij ~= "" then
		parts[#parts + 1] = "ZELLIJ=" .. ya.quote(zellij)
	end
	if session and session ~= "" then
		parts[#parts + 1] = "ZELLIJ_SESSION_NAME=" .. ya.quote(session)
	end
	if pane and pane ~= "" then
		parts[#parts + 1] = "ZELLIJ_PANE_ID=" .. ya.quote(pane)
	end

	local z_env = ""
	if #parts > 0 then
		z_env = table.concat(parts, " ") .. " "
	end
	local pane_id = os.getenv("ZELLIJ_PANE_ID") or ""
	local pane_arg = ""
	if pane_id ~= "" then
		pane_arg = " " .. ya.quote(pane_id)
	end
	os.execute(string.format("%s aoc-pane-rename %s%s &", z_env, ya.quote(title), pane_arg))
end

-- Debouncing logic
local last_request_time = 0
local pending_title = nil
local last_applied_title = nil

local function update_title()
	pending_title = resolve_title()
	last_request_time = ya.time()
	
	-- We use a single async task to "wait" for the settle
	ya.async(function()
		ya.sleep(0.15) -- 150ms debounce
		if ya.time() - last_request_time >= 0.14 then
			local final_title = resolve_title()
			if final_title ~= last_applied_title then
				last_applied_title = final_title
				do_rename(final_title)
			end
		end
	end)
end

return {
	entry = function()
		ps.sub("aoc-title-refresh", function() 
			last_applied_title = nil
			update_title() 
		end)
		-- Initial update without delay
		local initial_title = resolve_title()
		last_applied_title = initial_title
		do_rename(initial_title)
	end,
	setup = function()
		ya.emit("plugin", { "aoc-title" })
	end
}
