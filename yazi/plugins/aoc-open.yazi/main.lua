--- @sync entry
local M = {}

local function sh_quote(value)
	return "'" .. value:gsub("'", "'\\''") .. "'"
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

	local editor = "micro"
	ya.emit("shell", { string.format("%s %s", editor, sh_quote(url_str)), block = true })
end

return M
