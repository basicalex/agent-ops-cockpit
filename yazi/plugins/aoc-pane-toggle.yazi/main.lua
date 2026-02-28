--- @sync entry
local M = {}

local function ratio(parent, current, preview)
	return {
		parent,
		current,
		preview,
		parent = parent,
		current = current,
		preview = preview,
	}
end

local compact_ratio = ratio(0, 1, 0)
local full_ratio = ratio(1, 4, 3)

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

local function ensure_state_dir()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	os.execute(string.format("mkdir -p %s", ya.quote(state_root .. "/aoc")))
end

local function is_expanded(path)
	local f = io.open(path, "r")
	if not f then return false end
	f:close()
	return true
end

local function set_expanded(path, enabled)
	if enabled then
		ensure_state_dir()
		local f = io.open(path, "w")
		if f then f:write("1\n") f:close() end
		return
	end
	os.remove(path)
end

local function ratio_equals(value, target)
	if type(value) ~= "table" or type(target) ~= "table" then
		return false
	end

	if value[1] ~= nil then
		return value[1] == target[1] and value[2] == target[2] and value[3] == target[3]
	end

	return value.parent == target.parent and value.current == target.current and value.preview == target.preview
end

local function set_ratio(ratio)
	rt.mgr.ratio = ratio
	ya.emit("resize", {})
end

local function parse_steps(name, fallback)
	local value = tonumber(os.getenv(name) or "")
	if value == nil or value < 1 then return fallback end
	return math.floor(value)
end

function M:entry()
	local state_path = state_file_path()
	local expand_steps = parse_steps("AOC_YAZI_PANE_EXPAND_STEPS", 12)
	local shrink_steps = parse_steps("AOC_YAZI_PANE_SHRINK_STEPS", 11)

	local expanded = is_expanded(state_path)
	local current_ratio = rt and rt.mgr and rt.mgr.ratio or nil

	if expanded and ratio_equals(current_ratio, compact_ratio) then
		expanded = false
		set_expanded(state_path, false)
	elseif (not expanded) and ratio_equals(current_ratio, full_ratio) then
		expanded = true
		set_expanded(state_path, true)
	end

	if expanded then
		set_expanded(state_path, false)
		set_ratio(compact_ratio)
		local shrink = resize("decrease", shrink_steps)
		if shrink ~= "" then ya.emit("shell", { shrink, block = false }) end
		return
	end

	set_expanded(state_path, true)
	set_ratio(full_ratio)
	local expand = resize("increase", expand_steps)
	if expand ~= "" then ya.emit("shell", { expand, block = false }) end
	set_ratio(full_ratio)
end

return M
