local M = {}

local function render_message(job, message)
	message = message or "Mermaid preview unavailable"
	message = message:gsub("%s+", " ")
	if #message > 140 then
		message = message:sub(1, 137) .. "..."
	end

	ya.preview_widget(job, ui.Line(message):area(job.area))
end

local function helper_args(job)
	local url = tostring(job.file.url)
	local cols = tostring(math.max(20, job.area.w or 0))
	local rows = tostring(math.max(10, job.area.h or 0))
	local args = {
		"--input", url,
		"--cols", cols,
		"--rows", rows,
	}

	local block_index = os.getenv("AOC_YAZI_MERMAID_BLOCK_INDEX")
	if block_index ~= nil and block_index ~= "" then
		table.insert(args, "--block-index")
		table.insert(args, block_index)
	end

	return args
end

local function ensure_preview(job)
	local output, err = Command("aoc-yazi-mermaid")
		:arg(helper_args(job))
		:output()

	if err ~= nil then
		return nil, tostring(err)
	end

	if output == nil then
		return nil, "no preview output"
	end

	if not output.status.success then
		local stderr = output.stderr or ""
		if stderr == "" then
			stderr = string.format("helper exited with code %s", tostring(output.status.code))
		end
		return nil, stderr
	end

	local path = (output.stdout or ""):gsub("%s+$", "")
	if path == "" then
		return nil, "helper returned an empty preview path"
	end

	return path, nil
end

function M:preload(job)
	local path, err = ensure_preview(job)
	if path ~= nil then
		return true
	end
	return true, Err(err or "Mermaid preload failed")
end

function M:peek(job)
	local path, err = ensure_preview(job)
	if path == nil then
		render_message(job, "Mermaid: " .. (err or "preview failed"))
		return
	end

	ya.image_show(Url(path), job.area)
end

function M:seek() end

return M
