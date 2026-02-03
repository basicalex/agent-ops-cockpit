--- @sync entry
local M = {}

function M:entry()
	if not cx or not cx.active or not cx.active.current then return end
	local h = cx.active.current.hovered
	if not h then return end

	local url_str = tostring(h.url or "")
	if url_str == "" then return end

	local quoted = "'" .. url_str:gsub("'", "'\\''") .. "'"
	local cmd = string.format("aoc-open-explorer %s", quoted)

	ya.emit("shell", { cmd, block = false })
end

return M
