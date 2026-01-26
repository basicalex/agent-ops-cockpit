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

-- Initialize AOC title management by running the plugin
ya.emit("plugin", { "aoc-title" })
