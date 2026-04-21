# Bundled fonts for CanvasTerminal

Place redistributable font files in this folder for packaging.

Recommended files:

- `CaskaydiaCoveNerdFont-Regular.ttf` (preferred monospace Nerd Font)
- Optional alternatives:
  - `CaskaydiaCoveNerdFontMono-Regular.ttf`
  - `JetBrainsMonoNerdFont-Regular.ttf`
  - `MesloLGS NF Regular.ttf`

Packaging script (`scripts/package-canvas-app.cmd`) copies this folder to `dist/app/fonts/`.
At runtime CanvasTerminal searches fonts in:

1. `assets/fonts/` (dev run)
2. `<exe_dir>/fonts/` (packaged run)

Notes:

- Do **not** bundle `Segoe UI Emoji`; the app uses system fallback for emoji.
- Check each font's license before distribution.
