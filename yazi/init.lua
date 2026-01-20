-- AOC Yazi init.lua
-- Pulse Status Bar - Minimal Implementation
-- Overrides Status.redraw() for complete control

-- Override Status.redraw with our minimal Pulse bar
Status.redraw = function(self)
	-- Safety check
	if not self._area or not self._current then
		return {}
	end

	-- Simple help hint
	local left = ui.Line {
		ui.Span("  ? Help"):fg("white")
	}

	-- Return minimal status bar
	return {
		ui.Line(left):area(self._area),
	}
end
