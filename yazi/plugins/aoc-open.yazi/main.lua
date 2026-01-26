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

local function get_lock_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	return state_root .. "/aoc/yazi-edit.lock"
end

local function set_editing(enabled, filename)
	local path = get_lock_path()
	local z_env = string.format("ZELLIJ=%s ZELLIJ_SESSION_NAME=%s", 
		ya.quote(os.getenv("ZELLIJ") or ""), 
		ya.quote(os.getenv("ZELLIJ_SESSION_NAME") or ""))
		
	if enabled then
		local f = io.open(path, "w")
		if f then f:write(filename .. "\n") f:close() end
		os.execute(string.format("%s /home/ceii/dev/agent-ops-cockpit/bin/aoc-pane-rename %s &", z_env, ya.quote(filename)))
	else
		os.remove(path)
	end
end

function M:entry()
	-- 1. Get hovered item
	if not cx or not cx.active or not cx.active.current then return end
	local h = cx.active.current.hovered
	if not h then return end

	local url_str = tostring(h.url)
	
	-- 2. Directory Handling: Just enter
	if h.cha and h.cha.is_dir then
		ya.emit("enter", {})
		return
	end

	-- 3. Media Detection
	local name = url_str:match("([^/]+)$") or ""
	local ext = name:lower():match("%.([^%.]+)$")
	local media = {
		png=1, jpg=1, jpeg=1, gif=1, bmp=1, webp=1, svg=1, 
		mp4=1, mkv=1, webm=1, mov=1, avi=1, mp3=1, wav=1, flac=1
	}

	if ext and media[ext] then
		os.execute(string.format("aoc-widget-set '%s' &", url_str:gsub("'", "'\\''")))
		return
	end

	-- 4. Focus Edit Mode (Text/Code)
	local expand = resize("increase", 12)
	local shrink = resize("decrease", 11)
	
	local editor = os.getenv("EDITOR") or "micro"
	local quoted_url = "'" .. url_str:gsub("'", "'\\''") .. "'"
	
	-- Set editing state and title
	set_editing(true, name)
	
	-- Full Command: Expand -> Editor -> Shrink
	local cmd = string.format("%s && %s %s; %s", expand, editor, quoted_url, shrink)
	
	-- Use ya.emit("shell") with block=true to suspend Yazi TUI and fix lag
	ya.emit("shell", { cmd, block = true })
	
	-- Clear editing state
	set_editing(false)
	ps.pub("aoc-title-refresh", "")
end

return M
