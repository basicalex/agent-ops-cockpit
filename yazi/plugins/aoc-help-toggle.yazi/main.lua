--- @sync entry
local M = {}

local function in_zellij()
	return os.getenv("ZELLIJ") ~= nil or os.getenv("ZELLIJ_SESSION_NAME") ~= nil
end

local function resize(direction, steps)
	if not in_zellij() then return "" end
	return string.format("aoc-zellij-resize %s %d", direction, steps)
end

local function safe_id(value)
	if value == nil or value == "" then
		return "unknown"
	end
	return value:gsub("[^%w%-_]", "_")
end

local function pane_scope()
	local session = safe_id(os.getenv("ZELLIJ_SESSION_NAME") or "session")
	local pane = safe_id(os.getenv("ZELLIJ_PANE_ID") or "")
	if pane ~= "" then
		return session .. "-" .. pane
	end
	return session
end

local function state_file_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	return state_root .. "/aoc/yazi-pane-expanded-" .. pane_scope()
end

local function is_expanded(path)
	local f = io.open(path, "r")
	if not f then return false end
	f:close()
	return true
end

local function set_expanded(path, enabled)
	if enabled then
		local f = io.open(path, "w")
		if f then f:write("1\n") f:close() end
		return
	end
	os.remove(path)
end

function M:entry()
	local expand = resize("increase", 12)
	local state_path = state_file_path()
	local was_expanded = is_expanded(state_path)

	if not was_expanded and expand ~= "" then
		set_expanded(state_path, true)
		ya.emit("shell", { expand, block = true })
	end

	ya.emit("help", {})
end

return M
