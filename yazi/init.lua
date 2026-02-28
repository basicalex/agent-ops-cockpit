-- Pulse Status Bar - Minimal Implementation
Status.redraw = function(self)
	if not self._area or not self._current then
		return {}
	end

	local left = ui.Line {
		ui.Span("  ? Help"):fg("white")
	}

	return {
		ui.Line(left):area(self._area),
	}
end

function Entity:click(event, up)
	if up or event.is_middle then
		return
	end

	local was_hovered = self._file.is_hovered
	ya.emit("reveal", { self._file.url })

	if event.is_right then
		ya.emit("open", {})
		return
	end

	if was_hovered then
		ya.emit("plugin", { "aoc-open" })
	end
end

-- Pane rename/title automation deprecated (kept plugin in tree for compatibility).
