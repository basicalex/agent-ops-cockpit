--- @sync entry
local M = {}

local function in_zellij()
	return os.getenv("ZELLIJ") ~= nil
end

local function resize(direction, steps)
	if not in_zellij() then return "" end
	local step = "zellij action resize " .. direction .. " right >/dev/null 2>&1"
	local chain = {}
	for i = 1, steps do chain[i] = step end
	return table.concat(chain, " && ")
end

local function state_file_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	return state_root .. "/aoc/yazi-pane-expanded"
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
	local shrink = resize("decrease", 11)
	local state_path = state_file_path()

	ya.emit("help", {})

	if is_expanded(state_path) and shrink ~= "" then
		set_expanded(state_path, false)
		ya.emit("shell", { shrink, block = true })
	end
end

return M
