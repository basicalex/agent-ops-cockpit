## Design Mode: Dashboard

Focus on dense operational interfaces: dashboards, admin panels, consoles, dev tools, multi-tenant workspaces, and internal systems.

Use `enforce-dashboard-ux-guardrails` as the active dashboard standard. Keep output compact and implementation-ready:
- sidebar spine with top workspace selector, grouped navigation, and bottom utility drain
- top-right header action zone for filters, date ranges, and primary mutations
- dense tables/lists/cards with explicit loading, empty, error, hover, selected, disabled, and optimistic states
- popovers only for non-destructive contextual config; modals only for blocking heavy-input mutations
- row selection and bulk-action affordances for dense tables
- simple line/bar visualizations with axes, summaries, and hover behavior
