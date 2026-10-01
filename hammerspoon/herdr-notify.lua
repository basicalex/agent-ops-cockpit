-- Clickable macOS notifications for Herdr agents.
--
-- Herdr's own `system` delivery goes through terminal-notifier or osascript:
-- clicking the banner opens Script Editor, or at best raises the terminal
-- without moving to the pane. This module keeps one open `events.subscribe`
-- stream per running Herdr server, posts a notification when an agent finishes
-- or needs input in a tab you are not looking at, and on click focuses that
-- pane (workspace + tab) and raises the terminal app.
--
-- Pair it with `[ui.toast] delivery = "off"` in ~/.config/herdr/config.toml so
-- Herdr does not post a second banner. Sounds keep working; they are separate.
--
-- Manual test from a shell:  hs -c 'herdrNotify.test()'

local M = {}

M.config = {
  -- pane.list sweep: catches panes the stream missed and reopens dead streams.
  rescanSeconds = 60,
  -- Wait before reopening a stream that ended.
  reconnectSeconds = 5,
  -- Pane events arrive in bursts (split, close); coalesce the resubscribe.
  resyncDelaySeconds = 1,
  requestTimeoutSeconds = 3,
  retryDeadAfterSeconds = 300,
  ncPath = "/usr/bin/nc",
  terminalBundleID = "com.mitchellh.ghostty",
  herdrDir = os.getenv("HOME") .. "/.config/herdr",
  -- agent_status values that produce a notification, with the banner wording.
  notifyStates = { blocked = "needs input", done = "finished" },
  -- Banner wording for herdr agent ids; anything else is capitalised.
  agentNames = { claude = "Claude", codex = "Codex", omp = "omp", prime = "Prime", gemini = "Gemini" },
  -- App whose icon stands in as the notification sender, per agent id.
  agentBundleIDs = { claude = "com.anthropic.claudefordesktop" },
  iconScanDepth = 6,
  iconRescanSeconds = 600,
  -- Builder for the per-project notifier apps (bin/herdr-notifier-app). When it
  -- is missing the module falls back to hs.notify, which cannot set the sender
  -- icon or dismiss on click on macOS 26.
  notifierScript = {
    os.getenv("HOME") .. "/.local/bin/herdr-notifier-app",
    os.getenv("HOME") .. "/dev/agent-ops-cockpit/bin/herdr-notifier-app",
  },
}

-- socket path -> {
--   path, known = {pane_id -> agent_status}, primed, busy, failures,
--   stream = hs.task (the open nc subscription), buffer, reopenAt,
--   reopenTimer, resyncTimer, events, lastEventAt }
local sessions = {}
local liveNotifications = {} -- pane key -> hs.notify, kept referenced so click callbacks survive GC
local timers = {}
local deadUntil = {} -- socket path -> os.time() before which it is not retried

local function log(fmt, ...)
  print(string.format("[herdr-notify] " .. fmt, ...))
end

-- hs.socket crashes Hammerspoon 1.1.1 on macOS 26 when it connects to a unix
-- socket (GCDAsyncSocket connectedUrl -> NSURL fileURLWithPath:nil), so every
-- socket exchange goes through `nc -U` under hs.task instead. hs.task leaks a
-- little per task, so the design keeps task counts low: one long-lived nc per
-- server for the event stream, one short nc per sweep or notification.
--
-- hs.json.encode turns {} into "[]", which Herdr rejects for object params,
-- so callers pass params as a JSON string.
local inflight = {} -- hs.task -> true, kept referenced until the task ends

local function requestMany(sockPath, requests, callback)
  local lines = {}
  for i, r in ipairs(requests) do
    lines[i] = string.format('{"id":"hs%d","method":"%s","params":%s}', i, r[1], r[2])
  end
  local script = 'for line in "$@"; do printf "%s\\n" "$line" | '
    .. M.config.ncPath .. ' -U "' .. sockPath .. '" -w ' .. M.config.requestTimeoutSeconds .. '; done'
  local args = { "-c", script, "herdr-notify" }
  for _, l in ipairs(lines) do table.insert(args, l) end

  local task
  task = hs.task.new("/bin/sh", function(exitCode, stdout, stderr)
    inflight[task] = nil
    local results = {}
    for line in (stdout or ""):gmatch("[^\n]+") do
      local ok, decoded = pcall(hs.json.decode, line)
      if ok and type(decoded) == "table" then table.insert(results, decoded) end
    end
    if #results ~= #requests then
      local why = (stderr and stderr ~= "" and stderr:gsub("%s+$", "")) or ("exit " .. tostring(exitCode))
      callback(nil, why)
    else
      callback(results)
    end
  end, args)
  inflight[task] = true
  if not task:start() then
    inflight[task] = nil
    callback(nil, "spawn failed")
  end
end

local function request(sockPath, method, paramsJSON, callback)
  requestMany(sockPath, { { method, paramsJSON } }, function(results, err)
    callback(results and results[1] or nil, err)
  end)
end

local function terminalIsFrontmost()
  local app = hs.application.frontmostApplication()
  return app ~= nil and app:bundleID() == M.config.terminalBundleID
end

local function focusPane(sockPath, paneId)
  request(sockPath, "pane.focus", hs.json.encode({ pane_id = paneId }), function(result, err)
    if err then
      log("pane.focus %s failed: %s", paneId, err)
    end
    hs.application.launchOrFocusByBundleID(M.config.terminalBundleID)
  end)
end

-- Banner layout:
--   sender icon   the agent's app icon (Claude.app for claude, else the terminal)
--   title         the chat name (terminal title), else the tab label
--   subtitle      "Claude needs input" / "Codex finished"
--   body          "workspace › tab"
--   right image   a project icon, when one exists (see projectIcon)
local imageCache = {}

local function appIcon(bundleID)
  if bundleID == nil then return nil end
  if imageCache[bundleID] == nil then
    imageCache[bundleID] = hs.image.imageFromAppBundle(bundleID) or false
  end
  return imageCache[bundleID] or nil
end

-- Project icon lookup. The first source that yields a file wins:
--   ~/.config/herdr/icons/<workspace label>.(png|svg)   per-project override
--   <repo>/.herdr/icon.(png|svg)                          checked into the repo
--   the icon the repo's web app uses: app/icon.svg, public/favicon.svg,
--   public/brand/*-icon.svg, apple-touch-icon, icon-512, favicon.ico, logo.svg …
-- The repo is the nearest ancestor of the pane's cwd that holds a .git entry.
-- Scans run with find(1) under hs.task and are cached per repo.
local iconCache = {} -- repo root -> { path = string|false, at = os.time() }
local iconScans = {} -- repo root -> list of callbacks waiting on a scan

local function fileExists(path)
  return hs.fs.attributes(path, "mode") == "file"
end

local function repoRoot(cwd)
  local dir = cwd
  while dir and dir ~= "/" and dir ~= "" do
    if hs.fs.attributes(dir .. "/.git") then return dir end
    dir = dir:match("^(.*)/[^/]+$")
  end
  return nil
end

local function iconRank(root, path)
  local rel = path:sub(#root + 2):lower()
  local name = rel:match("[^/]+$") or rel
  local rank = 90
  if rel:match("^%.herdr/") then rank = 0
  elseif name == "icon.svg" or name == "icon.png" then rank = 10
  elseif name == "favicon.svg" or name == "favicon.png" then rank = 20
  elseif name:match("%-icon%.svg$") or name:match("%-icon%-[%w]+%.svg$") then rank = 30
  elseif name:match("^apple%-touch%-icon") then rank = 40
  elseif name:match("^icon%-512") then rank = 45
  elseif name:match("^icon%-192") then rank = 50
  elseif name == "favicon.ico" then rank = 60
  elseif name:match("^logo%-icon") then rank = 70
  elseif name == "logo.svg" or name == "logo.png" then rank = 80
  end
  if rel:match("dark") then rank = rank + 3 end
  if rel:match("maskable") then rank = rank + 2 end
  local _, depth = rel:gsub("/", "")
  return rank * 100 + depth
end

local PRUNE = {
  "node_modules", ".git", "dist", "build", "out", ".next", ".turbo", "coverage",
  "storybook-static", ".design-sync", "design-lab", "docs", "hyperframes",
  ".playwright-mcp", ".aoc", ".pi", ".omp", "*-build", "*-qa-build", "out-shell", "*-library",
}
local NAMES = {
  "favicon.*", "icon.svg", "icon.png", "apple-touch-icon*.png", "icon-512*", "icon-192*",
  "*-icon.svg", "*-icon-*.svg", "logo-icon*", "logo.svg", "logo.png",
}

local function scanRepoIcon(root, callback)
  local cached = iconCache[root]
  if cached and os.time() - cached.at < M.config.iconRescanSeconds then
    return callback(cached.path or nil)
  end
  if iconScans[root] then
    table.insert(iconScans[root], callback)
    return
  end
  iconScans[root] = { callback }

  local args = { root, "-maxdepth", tostring(M.config.iconScanDepth), "(" }
  for i, n in ipairs(PRUNE) do
    if i > 1 then table.insert(args, "-o") end
    table.insert(args, "-name"); table.insert(args, n)
  end
  table.insert(args, ")"); table.insert(args, "-prune"); table.insert(args, "-o")
  table.insert(args, "-type"); table.insert(args, "f"); table.insert(args, "(")
  for i, n in ipairs(NAMES) do
    if i > 1 then table.insert(args, "-o") end
    table.insert(args, "-iname"); table.insert(args, n)
  end
  table.insert(args, ")"); table.insert(args, "-print")

  local task
  task = hs.task.new("/usr/bin/find", function(_, stdout)
    inflight[task] = nil
    local best, bestRank = nil, math.huge
    for line in (stdout or ""):gmatch("[^\n]+") do
      local r = iconRank(root, line)
      if r < bestRank or (r == bestRank and line < best) then best, bestRank = line, r end
    end
    iconCache[root] = { path = best or false, at = os.time() }
    local waiting = iconScans[root]
    iconScans[root] = nil
    for _, cb in ipairs(waiting) do cb(best) end
  end, args)
  inflight[task] = true
  if not task:start() then
    inflight[task] = nil
    iconScans[root] = nil
    callback(nil)
  end
end

local function loadImage(path)
  if imageCache[path] == nil then
    imageCache[path] = hs.image.imageFromPath(path) or false
  end
  return imageCache[path] or nil
end

local function projectIcon(workspaceLabel, cwd, callback)
  if workspaceLabel then
    for _, ext in ipairs({ "png", "svg" }) do
      local override = M.config.herdrDir .. "/icons/" .. workspaceLabel .. "." .. ext
      if fileExists(override) then return callback(loadImage(override), override) end
    end
  end
  -- Only repos are scanned; a shell sitting in $HOME would otherwise trigger
  -- a find over the whole home directory.
  local root = cwd and repoRoot(cwd)
  if not root then return callback(nil) end
  scanRepoIcon(root, function(path)
    callback(path and loadImage(path) or nil, path)
  end)
end

local function agentName(agent)
  local name = agent.display_agent or agent.agent or "agent"
  return M.config.agentNames[name] or (name:sub(1, 1):upper() .. name:sub(2))
end

-- Pick the workspace and tab labels for a pane out of workspace.list and
-- tab.list responses.
local function labelsFor(agent, workspaceRes, tabRes)
  local workspaceLabel, tabLabel
  local ws = workspaceRes and workspaceRes.result and workspaceRes.result.workspaces
  for _, w in ipairs(ws or {}) do
    if w.workspace_id == agent.workspace_id then workspaceLabel = w.label end
  end
  local tabs = tabRes and tabRes.result and tabRes.result.tabs
  for _, t in ipairs(tabs or {}) do
    if t.tab_id == agent.tab_id then tabLabel = t.label end
  end
  return workspaceLabel, tabLabel
end

local function notifyLegacy(sess, agent, status, workspaceLabel, tabLabel, chat, where)
  local key = sess.path .. "|" .. agent.pane_id
  local previous = liveNotifications[key]
  if previous then
    pcall(previous.withdraw, previous)
    liveNotifications[key] = nil
  end
  local n
  n = hs.notify.new(function(clicked)
    liveNotifications[key] = nil
    pcall(clicked.withdraw, clicked)
    focusPane(sess.path, agent.pane_id)
  end, {
    title = chat,
    subTitle = string.format("%s %s", agentName(agent), M.config.notifyStates[status] or status),
    informativeText = where,
    withdrawAfter = 0,
    autoWithdraw = false,
    hasActionButton = true,
    actionButtonTitle = "Open",
  })
  projectIcon(workspaceLabel, agent.cwd, function(project)
    if project then n:contentImage(project) end
    liveNotifications[key] = n
    n:send()
  end)
end

-- Per-project notifier apps -------------------------------------------------
-- macOS shows the posting app's icon on the left of a banner, so each project
-- gets its own tiny .app (built by herdr-notifier-app) whose icon is the
-- project icon. Clicking the banner launches that app, which sends pane.focus
-- to Herdr and raises the terminal; UNUserNotificationCenter removes the banner.
local notifierScript
for _, candidate in ipairs(M.config.notifierScript) do
  if notifierScript == nil and hs.fs.attributes(candidate, "mode") == "file" then notifierScript = candidate end
end
local bundleCache = {} -- label|icon -> app path or false
local bundleBuilds = {} -- label|icon -> callbacks waiting on the build

local function notifierBundle(label, iconPath, callback)
  local key = label .. "|" .. (iconPath or "")
  if bundleCache[key] ~= nil then return callback(bundleCache[key] or nil) end
  if bundleBuilds[key] then
    table.insert(bundleBuilds[key], callback)
    return
  end
  bundleBuilds[key] = { callback }
  local args = { "make", label }
  if iconPath then table.insert(args, iconPath) end
  local task
  task = hs.task.new(notifierScript, function(exitCode, stdout, stderr)
    inflight[task] = nil
    local app = (stdout or ""):match("([^\n]+%.app)%s*$")
    if exitCode ~= 0 or not app then
      log("herdr-notifier-app make %s failed: %s", label, (stderr or ""):gsub("%s+$", ""))
      app = false
    end
    bundleCache[key] = app
    local waiting = bundleBuilds[key]
    bundleBuilds[key] = nil
    for _, cb in ipairs(waiting) do cb(app or nil) end
  end, args)
  inflight[task] = true
  if not task:start() then
    inflight[task] = nil
    bundleBuilds[key] = nil
    callback(nil)
  end
end

local function postViaNotifier(app, sess, agent, status, chat, where, onFailure)
  local key = sess.path .. "|" .. agent.pane_id
  local args = {
    "post",
    "--title", chat,
    "--subtitle", string.format("%s %s", agentName(agent), M.config.notifyStates[status] or status),
    "--socket", sess.path,
    "--pane", agent.pane_id,
    "--id", key,
  }
  if where ~= "" then table.insert(args, "--body"); table.insert(args, where) end
  local task
  task = hs.task.new(app .. "/Contents/MacOS/herdr-notifier", function(exitCode, _, stderr)
    inflight[task] = nil
    if exitCode ~= 0 then
      log("post via %s failed (exit %d): %s", app, exitCode, (stderr or ""):gsub("%s+$", ""))
      if onFailure then onFailure() end
    end
  end, args)
  inflight[task] = true
  if not task:start() then
    inflight[task] = nil
    if onFailure then onFailure() end
  end
end

local function notify(sess, agent, status, workspaceLabel, tabLabel)
  local chat = agent.terminal_title_stripped
  if chat == nil or chat == "" then chat = tabLabel or agent.pane_id end
  local parts = {}
  if workspaceLabel then table.insert(parts, workspaceLabel) end
  if tabLabel and tabLabel ~= chat then table.insert(parts, tabLabel) end
  local where = table.concat(parts, " › ")

  if not notifierScript then
    return notifyLegacy(sess, agent, status, workspaceLabel, tabLabel, chat, where)
  end
  local label = workspaceLabel or (agent.cwd and (repoRoot(agent.cwd) or agent.cwd):match("[^/]+$")) or "Herdr"
  projectIcon(workspaceLabel, agent.cwd, function(_, iconPath)
    notifierBundle(label, iconPath, function(app)
      -- A refused post (permission prompt not yet answered) still gets a
      -- plain Hammerspoon banner so nothing is missed.
      local function fallback() notifyLegacy(sess, agent, status, workspaceLabel, tabLabel, chat, where) end
      if app then
        postViaNotifier(app, sess, agent, status, chat, where, fallback)
      else
        fallback()
      end
    end)
  end)
end

-- Status tracking ------------------------------------------------------------
-- A pane's agent_status moved. Notify on a transition into a notifyStates
-- value when the pane's tab is not on screen. The event carries no tab or cwd,
-- so the visibility check and the labels come from one round trip.
local function onStatus(sess, paneId, status)
  local prev = sess.known[paneId]
  sess.known[paneId] = status
  if not sess.primed or prev == nil or prev == status then return end
  if not M.config.notifyStates[status] then return end
  requestMany(sess.path, {
    { "pane.current", "{}" }, { "agent.list", "{}" }, { "workspace.list", "{}" }, { "tab.list", "{}" },
  }, function(results, err)
    if not results then
      log("%s: lookup for %s failed: %s", sess.path, paneId, err or "?")
      return
    end
    local cur = results[1].result
    local focusedTab = cur and cur.pane and cur.pane.tab_id
    local agent
    for _, a in ipairs(results[2].result and results[2].result.agents or {}) do
      if a.pane_id == paneId then agent = a end
    end
    if not agent then
      log("%s %s -> %s: not in agent.list, skipped", sess.path, paneId, status)
      return
    end
    local visible = terminalIsFrontmost() and focusedTab ~= nil and focusedTab == agent.tab_id
    if visible then
      log("%s -> %s: tab on screen, skipped", paneId, status)
      return
    end
    local workspaceLabel, tabLabel = labelsFor(agent, results[3], results[4])
    log("%s -> %s: notifying (%s › %s)", paneId, status, workspaceLabel or "?", tabLabel or "?")
    notify(sess, agent, status, workspaceLabel, tabLabel)
  end)
end

local sync -- forward declaration

local function closeStream(sess)
  local task = sess.stream
  sess.stream = nil
  sess.buffer = ""
  if task then pcall(task.terminate, task) end
end

local function scheduleReopen(sess)
  if sess.reopenTimer then sess.reopenTimer:stop() end
  sess.reopenTimer = hs.timer.doAfter(M.config.reconnectSeconds, function()
    sess.reopenTimer = nil
    if sessions[sess.path] == sess then sync(sess) end
  end)
end

-- Pane set changed; resubscribe once the burst settles.
local function scheduleResync(sess)
  if sess.resyncTimer then return end
  sess.resyncTimer = hs.timer.doAfter(M.config.resyncDelaySeconds, function()
    sess.resyncTimer = nil
    if sessions[sess.path] == sess then sync(sess) end
  end)
end

local function handleStreamLine(sess, line)
  local ok, msg = pcall(hs.json.decode, line)
  if not ok or type(msg) ~= "table" then return end
  if msg.id == "stream" then
    if msg.error then
      log("%s: subscribe refused: %s", sess.path, msg.error.message or "?")
      closeStream(sess)
      scheduleReopen(sess)
    end
    return
  end
  sess.events = (sess.events or 0) + 1
  sess.lastEventAt = os.time()
  local kind = msg.event or ""
  local data = msg.data or {}
  if kind == "pane.agent_status_changed" or kind == "pane_agent_status_changed" then
    if data.pane_id and data.agent_status then onStatus(sess, data.pane_id, data.agent_status) end
  elseif kind == "pane.closed" or kind == "pane_closed" then
    local id = data.pane_id or (data.pane and data.pane.pane_id)
    if id then sess.known[id] = nil end
    scheduleResync(sess)
  elseif kind == "pane.created" or kind == "pane_created"
    or kind == "pane.agent_detected" or kind == "pane_agent_detected" then
    scheduleResync(sess)
  end
end

-- One `nc -U` per server, held open: Herdr streams one JSON line per event.
-- nc exits when its stdin closes, so the request goes in through setInput and
-- the pipe stays open until the task is terminated.
local function openStream(sess)
  closeStream(sess)
  local subs = { '{"type":"pane.created"}', '{"type":"pane.closed"}', '{"type":"pane.agent_detected"}' }
  for id in pairs(sess.known) do
    table.insert(subs, string.format('{"type":"pane.agent_status_changed","pane_id":"%s"}', id))
  end
  local req = '{"id":"stream","method":"events.subscribe","params":{"subscriptions":[' .. table.concat(subs, ",") .. "]}}\n"
  local task
  task = hs.task.new(M.config.ncPath, function(exitCode)
    if sess.stream ~= task then return end
    sess.stream = nil
    if sessions[sess.path] == sess then
      log("%s: stream ended (exit %s); reconnecting", sess.path, tostring(exitCode))
      scheduleReopen(sess)
    end
  end, function(_, stdout)
    if sess.stream ~= task then return false end
    sess.buffer = sess.buffer .. (stdout or "")
    while true do
      local line, rest = sess.buffer:match("^([^\n]*)\n(.*)$")
      if not line then break end
      sess.buffer = rest
      if line ~= "" then handleStreamLine(sess, line) end
    end
    return true
  end, { "-U", sess.path })
  sess.buffer = ""
  sess.stream = task
  if not task:start() then
    sess.stream = nil
    log("%s: could not start stream", sess.path)
    scheduleReopen(sess)
    return
  end
  task:setInput(req)
  sess.subscribed = #subs - 3
end

-- pane.list sweep: seeds the status map, notifies on anything the stream
-- missed, and (re)opens the stream when the pane set changed or it is down.
sync = function(sess)
  if sess.busy then return end
  sess.busy = true
  request(sess.path, "pane.list", "{}", function(res, err)
    sess.busy = false
    if sessions[sess.path] ~= sess then return end
    local panes = res and res.result and res.result.panes
    if not panes then
      sess.failures = sess.failures + 1
      if sess.failures >= 3 then
        local why = err or (res and res.error and res.error.message) or "no result"
        log("%s: %s; retrying in %ds", sess.path, why, M.config.retryDeadAfterSeconds)
        deadUntil[sess.path] = os.time() + M.config.retryDeadAfterSeconds
        closeStream(sess)
        sessions[sess.path] = nil
      end
      return
    end
    sess.failures = 0
    local seen, changed = {}, false
    for _, p in ipairs(panes) do
      seen[p.pane_id] = true
      if sess.known[p.pane_id] == nil then changed = true end
      if sess.primed then
        onStatus(sess, p.pane_id, p.agent_status)
      else
        sess.known[p.pane_id] = p.agent_status
      end
    end
    for id in pairs(sess.known) do
      if not seen[id] then
        sess.known[id] = nil
        changed = true
      end
    end
    sess.primed = true
    if changed or not sess.stream then openStream(sess) end
  end)
end

local function socketPaths()
  local paths = {}
  local dir = M.config.herdrDir
  local function add(path)
    if hs.fs.attributes(path, "mode") == "socket" then table.insert(paths, path) end
  end
  add(dir .. "/herdr.sock")
  local sessionsDir = dir .. "/sessions"
  if hs.fs.attributes(sessionsDir, "mode") == "directory" then
    for name in hs.fs.dir(sessionsDir) do
      if name ~= "." and name ~= ".." then
        add(sessionsDir .. "/" .. name .. "/herdr.sock")
      end
    end
  end
  return paths
end

local function rescan()
  local present = {}
  for _, path in ipairs(socketPaths()) do
    present[path] = true
    local dead = deadUntil[path]
    if dead and dead > os.time() then
      -- skip until the retry time
    elseif not sessions[path] then
      deadUntil[path] = nil
      sessions[path] = { path = path, known = {}, primed = false, busy = false, failures = 0, buffer = "" }
      log("watching %s", path)
    end
  end
  for path, sess in pairs(sessions) do
    if not present[path] then
      log("%s: socket gone", path)
      closeStream(sess)
      sessions[path] = nil
    else
      sync(sess)
    end
  end
end

function M.start()
  M.stop()
  rescan()
  timers.rescan = hs.timer.doEvery(M.config.rescanSeconds, rescan)
  return M
end

function M.stop()
  for k, t in pairs(timers) do
    t:stop()
    timers[k] = nil
  end
  for _, sess in pairs(sessions) do
    if sess.reopenTimer then sess.reopenTimer:stop() end
    if sess.resyncTimer then sess.resyncTimer:stop() end
    closeStream(sess)
  end
  sessions = {}
end

-- Post a banner for the currently focused pane so the click path can be checked.
function M.test()
  -- Prefer the session with the most agents; the default socket may be a stale server.
  local path, best = nil, -1
  for p, sess in pairs(sessions) do
    local n = 0
    for _ in pairs(sess.known) do n = n + 1 end
    if n > best then path, best = p, n end
  end
  path = path or socketPaths()[1]
  if not path then
    log("no herdr socket found")
    return
  end
  requestMany(path, { { "pane.current", "{}" }, { "workspace.list", "{}" }, { "tab.list", "{}" } }, function(results, err)
    local pane = results and results[1].result and results[1].result.pane
    if not pane then
      log("pane.current failed: %s", err or "no pane")
      return
    end
    local workspaceLabel, tabLabel = labelsFor(pane, results[2], results[3])
    notify({ path = path }, pane, "done", workspaceLabel, tabLabel)
    log("test banner sent for %s; click it to focus that pane", pane.pane_id)
  end)
end

-- Print which icon file a directory resolves to (repo root is found from it).
function M.icon(cwd)
  projectIcon(nil, cwd, function(image, path)
    log("icon for %s: %s", cwd, path or "none")
  end)
end

function M.live()
  return liveNotifications
end

function M.status()
  local live = 0
  for _ in pairs(liveNotifications) do live = live + 1 end
  log("%d notification(s) still shown", live)
  for path, sess in pairs(sessions) do
    local n = 0
    for _ in pairs(sess.known) do n = n + 1 end
    local pid = sess.stream and sess.stream:pid() or "-"
    log("%s: %d panes tracked, %d subscribed, stream pid %s, %d events (last %s), failures=%d",
      path, n, sess.subscribed or 0, tostring(pid), sess.events or 0,
      sess.lastEventAt and os.date("%H:%M:%S", sess.lastEventAt) or "never", sess.failures)
  end
end

return M
