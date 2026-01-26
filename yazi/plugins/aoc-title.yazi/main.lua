--- @sync entry
local function get_lock_path()
	local state_root = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
	return state_root .. "/aoc/yazi-edit.lock"
end

local function is_editing()
	local f = io.open(get_lock_path(), "r")
	if f then
		f:close()
		return true
	end
	return false
end

local function basename(path)
	local s = tostring(path or "")
	if s == "" then return "" end
	if s ~= "/" then s = s:gsub("/+$", "") end
	local name = s:match("([^/]+)$") or s
	return name == "" and "/" or name
end

local function build_hover_title()
	if not cx or not cx.active or not cx.active.current then
		return "Files"
	end
	
	local h = cx.active.current.hovered
	if h then
		local name = basename(h.url)
		if h.cha and h.cha.is_dir then
			return name .. "/"
		end
		return name
	end
	
	local cwd = cx.active.current.cwd
	if cwd then
		local name = basename(cwd)
		return name == "" and "/" or (name .. "/")
	end
	
	return "Files"
end

local function do_rename(title)
	local z_env = string.format("ZELLIJ=%s ZELLIJ_SESSION_NAME=%s", 
		ya.quote(os.getenv("ZELLIJ") or ""), 
		ya.quote(os.getenv("ZELLIJ_SESSION_NAME") or ""))
	os.execute(string.format("%s /home/ceii/dev/agent-ops-cockpit/bin/aoc-pane-rename %s &", z_env, ya.quote(title)))
end

-- Debouncing logic
local last_request_time = 0
local pending_title = nil
local last_applied_title = nil

local function update_title()
	if is_editing() then return end
	
	pending_title = build_hover_title()
	last_request_time = ya.time()
	
	-- We use a single async task to "wait" for the settle
	ya.async(function()
		ya.sleep(0.15) -- 150ms debounce
		if ya.time() - last_request_time >= 0.14 then
			if pending_title and pending_title ~= last_applied_title then
				last_applied_title = pending_title
				do_rename(pending_title)
			end
		end
	end)
end

return {
	entry = function()
		ps.sub("hover", function() update_title() end)
		ps.sub("cd", function() update_title() end)
		ps.sub("aoc-title-refresh", function() 
			last_applied_title = nil
			update_title() 
		end)
		-- Initial update without delay
		last_applied_title = build_hover_title()
		do_rename(last_applied_title)
	end,
	setup = function()
		ya.emit("plugin", { "aoc-title" })
	end
}
