local M = {}

local compact = { 0, 1, 0 }
local wide = { 0, 3, 2 }

local function is_compact(ratio)
	if type(ratio) ~= "table" then
		return false
	end
	if ratio[1] ~= nil then
		return ratio[1] == compact[1] and ratio[2] == compact[2] and ratio[3] == compact[3]
	end
	return ratio.parent == compact[1] and ratio.current == compact[2] and ratio.preview == compact[3]
end

function M:entry()
	if is_compact(rt.mgr.ratio) then
		rt.mgr.ratio = wide
	else
		rt.mgr.ratio = compact
	end
	ya.emit("resize")
end

return M
