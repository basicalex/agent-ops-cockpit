-- Linux muscle-memory shortcuts adapted for macOS.
-- Chrome bindings are enabled only while a Chrome-family app is frontmost.
-- Command+U/I/O/P form the tab layer; Command+Shift+O/P preserve Open/Print,
-- and Command+Comma/Period navigate back/forward in the current tab.

require("hs.ipc")
hs.ipc.cliInstall(os.getenv("HOME") .. "/.local", true)

local chromeBundleIDs = {
  ["com.google.Chrome"] = true,
  ["com.google.Chrome.beta"] = true,
  ["com.google.Chrome.canary"] = true,
  ["com.google.Chrome.dev"] = true,
}

-- Keep these references global so Hammerspoon's Lua garbage collector cannot
-- discard the application watcher and leave Chrome-only hotkeys enabled.
chromeHotkeys = {}

local function chromeHotkey(inputModifiers, inputKey, outputModifiers, outputKey)
  local hotkey = hs.hotkey.new(inputModifiers, inputKey, function()
    hs.eventtap.keyStroke(outputModifiers, outputKey, 0)
  end)
  table.insert(chromeHotkeys, hotkey)
end

local function chromeMenuHotkey(inputModifiers, inputKey, menuPath)
  local hotkey = hs.hotkey.new(inputModifiers, inputKey, function()
    local app = hs.application.frontmostApplication()
    if app ~= nil and chromeBundleIDs[app:bundleID()] == true then
      app:selectMenuItem(menuPath)
    end
  end)
  table.insert(chromeHotkeys, hotkey)
end

-- Chrome tab movement and navigation.
chromeHotkey({ "cmd" }, "u", { "ctrl", "shift" }, "pageup")
chromeMenuHotkey({ "cmd" }, "i", { "Tab", "Select Previous Tab" })
chromeMenuHotkey({ "cmd" }, "o", { "Tab", "Select Next Tab" })
chromeHotkey({ "cmd" }, "p", { "ctrl", "shift" }, "pagedown")
chromeMenuHotkey({ "cmd", "shift" }, "o", { "File", "Open File…" })
chromeMenuHotkey({ "cmd", "shift" }, "p", { "File", "Print…" })
chromeHotkey({ "cmd" }, ",", { "cmd" }, "[")
chromeHotkey({ "cmd" }, ".", { "cmd" }, "]")

local function setChromeHotkeys(enabled)
  for _, hotkey in ipairs(chromeHotkeys) do
    if enabled then
      hotkey:enable()
    else
      hotkey:disable()
    end
  end
end

local function isChrome(app)
  return app ~= nil and chromeBundleIDs[app:bundleID()] == true
end

chromeWatcher = hs.application.watcher.new(function(_, event, app)
  if event == hs.application.watcher.activated then
    setChromeHotkeys(isChrome(app))
  end
end)
chromeWatcher:start()
setChromeHotkeys(isChrome(hs.application.frontmostApplication()))

-- Linux Super+I/O equivalent for macOS Spaces. Navigate the ordered Spaces
-- directly instead of synthesizing Control+Left/Right, which depends on the
-- current Mission Control shortcut settings and can be swallowed.
local function moveDesktop(delta)
  local window = hs.window.frontmostWindow()
  local screen = (window and window:screen()) or hs.mouse.getCurrentScreen() or hs.screen.mainScreen()
  local currentSpace = hs.spaces.focusedSpace()
  local spaces = hs.spaces.spacesForScreen(screen) or {}

  for index, spaceID in ipairs(spaces) do
    if spaceID == currentSpace then
      local targetSpace = spaces[index + delta]
      if targetSpace ~= nil then
        hs.spaces.gotoSpace(targetSpace)
      end
      return
    end
  end
end

hs.hotkey.bind({ "ctrl" }, "i", function()
  moveDesktop(-1)
end)

hs.hotkey.bind({ "ctrl" }, "o", function()
  moveDesktop(1)
end)

-- The physical microphone/Dictation key is remapped to virtual F13 at login.
-- Toggle whichever input device macOS currently treats as the default.
local microphoneMuteIndicator = hs.menubar.new(false, "basicalexMicrophoneMuteIndicator")
if microphoneMuteIndicator ~= nil then
  microphoneMuteIndicator:setTitle(hs.styledtext.new("●", {
    color = { red = 1, green = 0.12, blue = 0.10, alpha = 1 },
    font = { size = 13 },
  }))
  microphoneMuteIndicator:setTooltip("Microphone muted")
end

local function updateMicrophoneMuteIndicator(muted)
  if microphoneMuteIndicator == nil then
    return
  end

  if muted then
    if not microphoneMuteIndicator:isInMenuBar() then
      microphoneMuteIndicator:returnToMenuBar()
    end
  elseif microphoneMuteIndicator:isInMenuBar() then
    microphoneMuteIndicator:removeFromMenuBar()
  end
end

local function toggleMicrophoneMute()
  local inputDevice = hs.audiodevice.defaultInputDevice()
  if inputDevice == nil then
    updateMicrophoneMuteIndicator(false)
    hs.alert.show("No microphone found")
    return
  end

  local muted = inputDevice:inputMuted()
  if muted == nil then
    updateMicrophoneMuteIndicator(false)
    hs.alert.show("Microphone mute unavailable")
    return
  end

  local nextMuted = not muted
  if inputDevice:setInputMuted(nextMuted) then
    updateMicrophoneMuteIndicator(nextMuted)
    hs.alert.show(nextMuted and "Microphone muted" or "Microphone on")
  else
    hs.alert.show("Microphone mute unavailable")
  end
end

hs.hotkey.bind({}, "f13", toggleMicrophoneMute)

local currentInputDevice = hs.audiodevice.defaultInputDevice()
updateMicrophoneMuteIndicator(currentInputDevice ~= nil and currentInputDevice:inputMuted() == true)

-- Clickable Herdr agent notifications (see hammerspoon/herdr-notify.lua).
herdrNotify = require("herdr-notify").start()

hs.alert.show("AOC shortcuts loaded")
