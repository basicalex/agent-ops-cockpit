-- Linux muscle-memory shortcuts adapted for macOS.
-- Chrome bindings are enabled only while a Chrome-family app is frontmost,
-- leaving the same Option keys available to Herdr in the terminal.

require("hs.ipc")
hs.ipc.cliInstall(os.getenv("HOME") .. "/.local", true)

local chromeBundleIDs = {
  ["com.google.Chrome"] = true,
  ["com.google.Chrome.beta"] = true,
  ["com.google.Chrome.canary"] = true,
  ["com.google.Chrome.dev"] = true,
}

local chromeHotkeys = {}

local function chromeHotkey(inputKey, outputModifiers, outputKey)
  local hotkey = hs.hotkey.new({ "alt" }, inputKey, function()
    hs.eventtap.keyStroke(outputModifiers, outputKey, 0)
  end)
  table.insert(chromeHotkeys, hotkey)
end

local function chromeMenuHotkey(inputKey, menuItem)
  local hotkey = hs.hotkey.new({ "alt" }, inputKey, function()
    local app = hs.application.frontmostApplication()
    if app ~= nil and chromeBundleIDs[app:bundleID()] == true then
      app:selectMenuItem({ "Tab", menuItem })
    end
  end)
  table.insert(chromeHotkeys, hotkey)
end

-- Chrome tab movement and navigation.
chromeHotkey("u", { "ctrl", "shift" }, "pageup")
chromeMenuHotkey("i", "Select Previous Tab")
chromeMenuHotkey("o", "Select Next Tab")
chromeHotkey("p", { "ctrl", "shift" }, "pagedown")
chromeHotkey(",", { "cmd" }, "[")
chromeHotkey(".", { "cmd" }, "]")

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

local chromeWatcher = hs.application.watcher.new(function(_, event, app)
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

-- The MacBook microphone/Dictation hardware key is remapped to virtual F13 by
-- com.basicalex.macshot-mic-key-remap, leaving Fn+microphone available as F5.
hs.hotkey.bind({}, "f13", function()
  hs.urlevent.openURL("macshot://capture")
end)

hs.alert.show("AOC shortcuts loaded")
