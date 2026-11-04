# Usage panel in the Panorama layout: author capture

Operator request (main): "In panorama I still need that usage panel on the right as it was".

[Usage panel shown](native-usage-panel.png) · [agent-usage absent](native-usage-absent.png) · [provenance](provenance.json)

Both captures are 1600×1000 windows at zoom 1.2 from the working tree whose
`app/mods/panorama-tabs/main.js` and `app/layouts/panorama.json` match the SHA256
values in `provenance.json` (the committed files of this PR).

How they were made: a temporary `HERDR_APP_HOME`, `HOME` and XDG dirs and a private
`hc` core (0 live terminals at start), with the six panorama mods and the packaged
layout copied in, then `layout.switch panorama` inside that isolated window only.

- **Present:** agent-usage's own `main.js`, `mod.json` and `usage.svg` (read-only
  copies, no `local.json`) sit in the scratch home, fed by a synthetic `usagebar`
  script that prints a fixed two-account report (`demo@example.com`). No personal
  usage data was read. `herdr-app status` errors: `[]`.
- **Absent, idle:** no `mods/agent-usage`. The Usage toggle is hidden; status
  errors `[]` (the hidden stack child is never loaded).
- **Absent, forced** (the image): `layout.select usage` + `layout.show panel`
  simulate a panel remembered from a layout that had the mod. panorama-tabs closes
  the panel; the image shows no toggle and no empty panel. Herdr itself still logs
  `mod agent-usage: no mod folder ...` in status for that attempted load (the path
  shown is the app's display name for the isolated home).

The "Updated to 0.1.13" toast is the isolated app's first-run notice.

Installer ownership: an isolated install then uninstall into a temp app home that
already had a synthetic `mods/agent-usage` left those files byte-identical, and the
receipt names no agent-usage path (the inventory is the fixed panorama `MODS` list).
