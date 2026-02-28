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

local function in_zellij()
	return os.getenv("ZELLIJ") ~= nil or os.getenv("ZELLIJ_SESSION_NAME") ~= nil
end

local function resize(direction, steps)
	if not in_zellij() then return "" end
	return string.format("aoc-zellij-resize %s %d", direction, steps)
end

local function ratio_components(value)
	if type(value) ~= "table" then
		return nil, nil, nil
	end

	local parent = value[1]
	local current = value[2]
	local preview = value[3]

	if parent == nil then parent = value.parent end
	if current == nil then current = value.current end
	if preview == nil then preview = value.preview end

	if parent == nil or current == nil or preview == nil then
		return nil, nil, nil
	end

	return parent, current, preview
end

local function ratio_equals(value, target)
	local vp, vc, vv = ratio_components(value)
	local tp, tc, tv = ratio_components(target)
	if vp == nil or tp == nil then
		return false
	end
	return vp == tp and vc == tc and vv == tv
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

local function parse_steps(name, fallback)
	local value = tonumber(os.getenv(name) or "")
	if value == nil or value < 1 then return fallback end
	return math.floor(value)
end

local function parse_bool(name, fallback)
	local value = os.getenv(name)
	if value == nil or value == "" then return fallback end
	value = value:lower()
	return value == "1" or value == "true" or value == "yes" or value == "on"
end

local function sh_quote(value)
	return "'" .. value:gsub("'", "'\\''") .. "'"
end

local function should_resize_for_edit()
	if not in_zellij() then
		return false
	end

	local current_ratio = rt and rt.mgr and rt.mgr.ratio or nil
	if ratio_equals(current_ratio, compact_ratio) then
		return true
	end

	-- Primary fallback: infer from pane-expanded state file.
	-- If state says expanded, skip; otherwise expand.
	return not is_expanded(state_file_path())
end

function M:entry()
	if not cx or not cx.active or not cx.active.current then return end
	local hovered = cx.active.current.hovered
	if not hovered then return end

	local url_str = tostring(hovered.url)

	if hovered.cha and hovered.cha.is_dir then
		ya.emit("enter", {})
		return
	end

	local name = url_str:match("([^/]+)$") or ""
	local ext = name:lower():match("%.([^%.]+)$")
	local media = {
		png = true, jpg = true, jpeg = true, gif = true, bmp = true, webp = true, svg = true,
		mp4 = true, mkv = true, webm = true, mov = true, avi = true, mp3 = true, wav = true, flac = true,
	}

	if ext and media[ext] then
		os.execute(string.format("aoc-open-file %s &", sh_quote(url_str)))
		return
	end

	local editor = os.getenv("EDITOR") or "micro"
	local quoted_url = sh_quote(url_str)

	local should_resize = should_resize_for_edit()
	local expand_steps = parse_steps("AOC_YAZI_OPEN_EXPAND_STEPS", parse_steps("AOC_YAZI_PANE_EXPAND_STEPS", 12))
	local shrink_steps = parse_steps("AOC_YAZI_OPEN_SHRINK_STEPS", parse_steps("AOC_YAZI_PANE_SHRINK_STEPS", 11))
	local async_expand = parse_bool("AOC_YAZI_OPEN_ASYNC_EXPAND", true)

	local commands = {}
	if should_resize then
		local expand = resize("increase", expand_steps)
		if expand ~= "" then
			if async_expand then
				commands[#commands + 1] = string.format("(%s >/dev/null 2>&1 &)", expand)
			else
				commands[#commands + 1] = expand
			end
		end
	end

	commands[#commands + 1] = string.format("%s %s", editor, quoted_url)

	if should_resize then
		local shrink = resize("decrease", shrink_steps)
		if shrink ~= "" then
			commands[#commands + 1] = shrink
		end
	end

	ya.emit("shell", { table.concat(commands, "; "), block = true })
end

return M
