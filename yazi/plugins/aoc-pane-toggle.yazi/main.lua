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

local function env(name)
	local value = os.getenv(name)
	if value == nil or value == "" then return nil end
	return value
end

local function in_zellij()
	return env("ZELLIJ") ~= nil
		or env("ZELLIJ_SESSION_NAME") ~= nil
		or env("AOC_ZELLIJ_SESSION_NAME") ~= nil
end

local function spawn_resize(direction, steps)
	if not in_zellij() then return end

	local command = env("AOC_ZELLIJ_RESIZE_CMD") or ((env("HOME") or "") .. "/.local/bin/aoc-zellij-resize")
	local session = env("ZELLIJ_SESSION_NAME") or env("AOC_ZELLIJ_SESSION_NAME")
	local pane = env("ZELLIJ_PANE_ID") or env("AOC_ZELLIJ_PANE_ID")
	local zellij = env("ZELLIJ")
	local zellij_bin = env("AOC_ZELLIJ_BIN")

	-- New path: use Yazi's Command API instead of shell text. This avoids the
	-- delayed shell/task behavior that made the UI update only after another key.
	local ok = false
	if Command ~= nil then
		ok = pcall(function()
			local cmd = Command(command)
				:arg(direction)
				:arg(tostring(steps))
				:arg("right")
			if session ~= nil then cmd = cmd:env("AOC_ZELLIJ_SESSION_NAME", session) end
			if pane ~= nil then cmd = cmd:env("AOC_ZELLIJ_PANE_ID", pane) end
			if zellij ~= nil then cmd = cmd:env("ZELLIJ", zellij) end
			if zellij_bin ~= nil then cmd = cmd:env("AOC_ZELLIJ_BIN", zellij_bin) end
			cmd:spawn()
		end)
	end

	if ok then return end

	-- Compatibility fallback for older Yazi builds.
	local prefix = ""
	if session ~= nil then prefix = prefix .. "AOC_ZELLIJ_SESSION_NAME=" .. ya.quote(session) .. " " end
	if pane ~= nil then prefix = prefix .. "AOC_ZELLIJ_PANE_ID=" .. ya.quote(pane) .. " " end
	if zellij_bin ~= nil then prefix = prefix .. "AOC_ZELLIJ_BIN=" .. ya.quote(zellij_bin) .. " " end
	ya.emit("shell", { string.format("%s%s %s %d right", prefix, command, direction, steps), block = false })
end

local function safe_id(value)
	if value == nil or value == "" then
		return "unknown"
	end
	return value:gsub("[^%w%-_]", "_")
end

local function pane_scope()
	local session = safe_id(env("ZELLIJ_SESSION_NAME") or env("AOC_ZELLIJ_SESSION_NAME") or "session")
	local pane = safe_id(env("ZELLIJ_PANE_ID") or env("AOC_ZELLIJ_PANE_ID") or "")
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

local function render_now()
	if ui ~= nil and ui.render ~= nil then
		pcall(function() ui.render() end)
	end
end

local function set_ratio(value)
	rt.mgr.ratio = value
	ya.emit("resize", {})
	render_now()
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
		spawn_resize("decrease", shrink_steps)
		render_now()
		return
	end

	set_expanded(state_path, true)
	set_ratio(full_ratio)
	spawn_resize("increase", expand_steps)
	set_ratio(full_ratio)
	render_now()
end

return M
