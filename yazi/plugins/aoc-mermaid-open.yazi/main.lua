--- @sync entry
local M = {}

local function sh_quote(value)
	return "'" .. value:gsub("'", "'\\''") .. "'"
end

local function hovered_url()
	if not cx or not cx.active or not cx.active.current then return nil end
	local hovered = cx.active.current.hovered
	if not hovered then return nil end
	return tostring(hovered.url or "")
end

local function mermaid_file(path)
	local lower = path:lower()
	return lower:match("%.mmd$") ~= nil or lower:match("%.mermaid$") ~= nil
end

function M:entry()
	local url = hovered_url()
	if url == nil or url == "" then return end

	if not mermaid_file(url) then
		ya.emit("plugin", { "aoc-open" })
		return
	end

	os.execute(string.format("aoc-yazi-mermaid-open %s >/tmp/aoc-yazi-mermaid-open.log 2>&1 &", sh_quote(url)))
end

return M
