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
	-- Expand 12 steps for Focus, Shrink 11 steps to ensure it doesn't get too small
	local expand = resize("increase", 12)
	local shrink = resize("decrease", 11)
	
	local editor = os.getenv("EDITOR") or "micro"
	local quoted_url = "'" .. url_str:gsub("'", "'\\''") .. "'"
	
	-- Full Command: Expand -> Editor -> Shrink
	local cmd = string.format("%s && %s %s; %s", expand, editor, quoted_url, shrink)
	
	-- Use ya.emit("shell") with block=true to suspend Yazi TUI and fix lag
	ya.emit("shell", { cmd, block = true })
end

return M
