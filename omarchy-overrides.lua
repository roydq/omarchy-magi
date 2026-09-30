-- Put ~/.local/share/omarchy-overrides/bin ahead of Omarchy's own commands on
-- the PATH of everything Hyprland launches, so a command there (like MAGI,
-- linked as omarchy-screensaver) replaces Omarchy's. Omarchy puts its bin
-- directory first on PATH, so the overrides go just ahead of it.
--
-- Copy to ~/.config/hypr/ and add require("hypr.omarchy-overrides") to the end
-- of ~/.config/hypr/hyprland.lua.
local overrides = os.getenv("HOME") .. "/.local/share/omarchy-overrides/bin"
local omarchy_bin = (os.getenv("OMARCHY_PATH") or "/usr/share/omarchy") .. "/bin"
local path = { overrides, omarchy_bin }
for entry in (os.getenv("PATH") or "/usr/local/bin:/usr/bin"):gmatch("[^:]+") do
  if entry ~= overrides and entry ~= omarchy_bin then
    table.insert(path, entry)
  end
end
hl.env("PATH", table.concat(path, ":"))
