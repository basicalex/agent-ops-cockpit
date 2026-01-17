-- AOC Open Plugin for Yazi
-- Opens files and resizes Zellij pane to 40% for better visibility

--- @sync entry
local M = {}

-- Check if we're running in Zellij
local function in_zellij()
	return os.getenv("ZELLIJ") ~= nil
end

-- Resize Zellij pane to 40%
local function resize_pane()
	if not in_zellij() then
		return
	end
	-- Use zellij action to resize the focused pane
	-- First resize to ensure visibility
	os.execute("zellij action resize increase left 2>/dev/null")
	os.execute("zellij action resize increase left 2>/dev/null")
	os.execute("zellij action resize increase left 2>/dev/null")
end

function M:entry()
	local h = cx.active.current.hovered
	if not h then
		return
	end

	if h.cha.is_dir then
		-- For directories, just enter them
		ya.emit("enter", {})
	else
		-- For files, resize pane and open
		resize_pane()
		ya.emit("open", {})
	end
end

return M
