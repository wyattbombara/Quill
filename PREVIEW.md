# Quill frontend preview

[View Roundtable in Quill 0.2.0](screenshots/Quill-0.2.0-roundtable.png). The image shows three model seats streaming sample contributions.

[View inline deletion and an inset code scrollbar in Quill 0.1.9](screenshots/Quill-0.1.9-inline-delete.png). This screenshot uses sample data.

[View the Quill 0.1.8 delete confirmation](screenshots/Quill-0.1.8-delete.png), captured with sample conversation data. In the desktop app, use Settings → Motion to choose Follow system, Smooth or Reduced.

[View the Quill 0.1.7 conversation window](screenshots/Quill-0.1.7-chat.png), captured with sample data. Run `npm run tauri dev` to use the redesigned desktop app with local history. The browser routes below preserve the earlier design studies.

## Run

```sh
npm ci
npm run dev -- --host 0.0.0.0
```

Then open the Vite URL with one of these routes:

- `/?demo=1&theme=light` — backend-free sample conversation in light mode
- `/?demo=1&theme=dark` — backend-free sample conversation in dark mode
- `/?demo=1&theme=light&palette=Cobalt%20%2F%20apricot` — same full shell with an alternate palette
- `/?directions=1` — the two-direction comparison used before selection
- `/?palettes=1` — compact palette playground with all four alternate combinations
- `/` — the real Tauri-connected app flow

The demo uses the real `Transcript` and `Composer` components with sample data. It does not request an API key or add another provider. The real app continues to call the existing Rust commands from `src/ipc.ts`.

Use the **Color palette** selector in the demo toolbar to switch the full shell among Paper / ink / violet, Violet / clay, Cobalt / apricot, Moss / parchment, and Rosewood / oat. The appearance button switches light/dark independently.

## Checks

- `npm run build` passes.
- Browser interaction check: Enter sends and clears the demo draft; the appearance toggle switches light/dark mode.
- Streaming return behavior is visible as **Latest message** when the demo transcript is away from the bottom.
- Exact screenshots were captured at 1100 × 750 and 700 × 520 in both light and dark modes.
- Rust tests were not run because `cargo` is not installed in this sandbox; no Rust source was changed.
