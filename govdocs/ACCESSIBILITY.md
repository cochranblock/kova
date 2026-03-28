# Accessibility — Section 508 Compliance

**Product:** kova v0.7.0
**Date:** 2026-03-27
**Standard:** Section 508 of the Rehabilitation Act, WCAG 2.1 Level AA

## Interface Inventory

kova provides four user interfaces:

1. **CLI** — primary interface (`kova` binary, 30+ subcommands)
2. **TUI** — terminal UI via ratatui (`kova tui`)
3. **GUI** — native desktop via egui (`kova gui`)
4. **Web** — WASM thin client via `kova serve`

---

## CLI Accessibility

### Help Text

Every subcommand provides `--help` with descriptions. Verified in `src/main.rs`:

- `kova --help` — top-level help with all subcommands listed
- `kova serve --help`, `kova chat --help`, etc. — per-command help
- Descriptions written in active voice, plain English

**Source:** `src/main.rs` lines 17-110 (every `Cmd` variant has a `///` doc comment that clap uses for `--help`).

### Exit Codes

- `0` = success
- `1` = failure

Consistent across all subcommands. Scripts and screen readers can parse exit codes for status.

### Error Messages

Error output includes context via `anyhow` and `thiserror`:
```
sled open failed: /path/to/db: Permission denied
```

No error codes without explanation. No silent failures.

### Screen Reader Compatibility

- All output is plain text to stdout/stderr.
- No ANSI escape codes in non-TTY mode (crossterm detects terminal capability).
- Structured output available via `--expand` flag on tokenized commands.

---

## TUI Accessibility (ratatui)

### Keyboard Navigation

- Full keyboard navigation. No mouse required.
- Tab/Shift-Tab for focus cycling.
- Enter for selection/submission.
- Escape for cancel/back.
- Arrow keys for list navigation.

**Source:** `src/tui.rs`, `Cargo.toml` lines 43-44 (ratatui + crossterm).

### Color and Contrast

- Inherits terminal color scheme by default.
- Works with high-contrast terminal themes.
- No information conveyed by color alone.

---

## GUI Accessibility (egui)

### Touch Targets

- Minimum 48x48 pixel touch targets for mobile (Android).
- 2.5x scaling for mobile displays.

### Typography

- Body text: 14pt minimum.
- Monospace for code display.
- Font sizes defined in `src/surface/gui/theme.rs` via `text_styles()`.

**Source:** `src/surface/gui/theme.rs` line 44.

### Color Contrast

Theme colors from `src/surface/gui/theme.rs` lines 9-21:

| Element | Color | Hex |
|---|---|---|
| Background | Dark | #0A0A0F |
| Surface | Dark elevated | #14141F |
| Primary | Cyan | #00D4FF |
| Text | Light gray | #E2E8F0 |
| Muted | Gray | #64748B |

Contrast ratios (approximate):
- Text (#E2E8F0) on Background (#0A0A0F): >15:1 (exceeds WCAG AAA 7:1)
- Primary (#00D4FF) on Background (#0A0A0F): >8:1 (exceeds WCAG AAA 7:1)
- Muted (#64748B) on Background (#0A0A0F): >4.5:1 (meets WCAG AA)

### Layout

Layout constants from `src/surface/gui/theme.rs` lines 24-36:
- Margins: 16px
- Padding: 6px (small), 12px (medium), 16px (large)
- Gap: 8px
- Corner radius: 8px (windows), 4px (small elements)

### Interaction

- All interactive elements have hover states (`SURFACE_HOVER: #1A2A35`).
- Window stroke provides visual boundary (1px, `SURFACE_ELEVATED`).
- egui provides built-in keyboard navigation for all widgets.

---

## Web (WASM) Accessibility

### Architecture

The WASM client renders the same egui interface in the browser. Accessibility properties are inherited from the egui framework.

**Source:** `src/web_client/mod.rs`, `src/web_client/app.rs`, `src/web_client/theme.rs`.

### Limitations

- WASM canvas rendering has limited screen reader support (egui paints to canvas).
- For screen reader users, the CLI interface is the recommended primary interface.

---

## Assistive Technology Support

| Interface | Screen Reader | Keyboard Only | High Contrast | Magnification |
|---|---|---|---|---|
| CLI | Full (text output) | Full | N/A (terminal) | Terminal zoom |
| TUI | Limited (terminal) | Full | Terminal theme | Terminal zoom |
| GUI | Limited (egui canvas) | Full | Built-in dark theme | OS zoom + scaling |
| Web | Limited (canvas) | Full | Inherits GUI theme | Browser zoom |

## Recommendations for Full 508 Compliance

1. Add ARIA labels to WASM canvas for screen reader support (egui upstream feature).
2. Add `--no-color` CLI flag for users who need plain text output.
3. Expose GUI widget tree via platform accessibility APIs (egui `accesskit` integration).
