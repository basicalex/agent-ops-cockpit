-- AOC Yazi init.lua
-- Status bar with command tips (like Zellij status bar)

-- Catppuccin-inspired colors matching Zellij status bar
local green = "#A6E3A1"
local gray = "#6C7086"

-- Add command tips to the right side of the status bar
Status:children_add(function()
	return ui.Line({
		ui.Span(" Enter "):fg(green),
		ui.Span("Open+Expand"):fg(gray),
		ui.Span(" e "):fg(green),
		ui.Span("Edit"):fg(gray),
		ui.Span(" q "):fg(green),
		ui.Span("Quit"):fg(gray),
		ui.Span(" p "):fg(green),
		ui.Span("Preview"):fg(gray),
		ui.Span(" Ctrl+p "):fg(green),
		ui.Span("Toggle"):fg(gray),
		ui.Span(" S "):fg(green),
		ui.Span("Star"):fg(gray),
		ui.Span(" "):fg(gray),
	})
end, 5000, Status.RIGHT)
