--- @sync entry
local M = {}

local function in_zellij()
	return os.getenv("ZELLIJ") ~= nil or os.getenv("ZELLIJ_SESSION_NAME") ~= nil
end

local function zellij_env_prefix()
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

	if #parts == 0 then return "" end
	return table.concat(parts, " ") .. " "
end

local function resize(direction, steps)
	if not in_zellij() then return "" end
	return string.format("aoc-zellij-resize %s %d", direction, steps)
end

local function normalize_cmd(cmd)
	if cmd == "" then return ":" end
	return cmd
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

local function get_lock_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	return state_root .. "/aoc/yazi-edit-" .. pane_scope() .. ".lock"
end

local function ensure_state_dir()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	os.execute(string.format("mkdir -p %s", ya.quote(state_root .. "/aoc")))
end

local function rename_pane(title)
	local z_env = zellij_env_prefix()
	local pane_id = os.getenv("ZELLIJ_PANE_ID") or ""
	local pane_arg = ""
	if pane_id ~= "" then
		pane_arg = " " .. ya.quote(pane_id)
	end
	os.execute(string.format("%s aoc-pane-rename %s%s &", z_env, ya.quote(title), pane_arg))
end

local EMPTY_TITLE = " "

local function set_editing(filename)
	local path = get_lock_path()
	ensure_state_dir()
	local f = io.open(path, "w")
	if f then f:write(filename .. "\n") f:close() end
	rename_pane(filename)
	ps.pub("aoc-title-refresh", "")
end

local function clear_editing_cmd()
	local path = get_lock_path()
	local z_env = zellij_env_prefix()
	local pane_id = os.getenv("ZELLIJ_PANE_ID") or ""
	local pane_arg = ""
	if pane_id ~= "" then
		pane_arg = " " .. ya.quote(pane_id)
	end
	return string.format("rm -f %s; %s aoc-pane-rename %s%s >/dev/null 2>&1 || true; printf '\\033]0;\\007\\033]2;\\007'", ya.quote(path), z_env, ya.quote(EMPTY_TITLE), pane_arg)
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
		os.execute(string.format("aoc-open-file '%s' &", url_str:gsub("'", "'\\''")))
		return
	end

	-- 4. Focus Edit Mode (Text/Code)
	local expand = resize("increase", 12)
	local shrink = resize("decrease", 11)
	
	local editor = os.getenv("EDITOR") or "micro"
	local quoted_url = "'" .. url_str:gsub("'", "'\\''") .. "'"
	
	-- Set editing state and title
	set_editing(url_str)
	
	local expand_cmd = normalize_cmd(expand)
	local shrink_cmd = normalize_cmd(shrink)
	local cleanup_cmd = clear_editing_cmd()

	-- Full Command: Expand -> Editor -> Shrink -> Cleanup
	local cmd = string.format("%s; %s %s; %s; %s", expand_cmd, editor, quoted_url, shrink_cmd, cleanup_cmd)
	
	-- Use ya.emit("shell") with block=true to suspend Yazi TUI and fix lag
	ya.emit("shell", { cmd, block = true })

	-- Refresh title after editor exits and cleanup runs
	ps.pub("aoc-title-refresh", "")
	-- Clear terminal title in Yazi pane
	ya.emit("shell", { "printf '\\033]0;\\007\\033]2;\\007'", block = false })
	
end

return M
