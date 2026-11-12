# Plan: recreate the HTML features games need, in the engine's ECS UI

## Status (2026-11-11)
Phases P1 to P9 are implemented within the approved scope. `docs/milestone-3.md` lists every element with the test that proves it and every gap that is left. The dependencies of decisions 1 and 4 were approved (`accesskit` with its Windows adapter, `fontdue`, `rustybuzz`, `unicode-segmentation`, OGG and MP3 decoders, `roxmltree`) and are used. What stays open: running the Windows adapter on Windows (and a Linux or macOS adapter, not approved), multi-window tear-off (decision 6, deferred by decision) and real video decoding (decision 5, not to be built).

## Context
Morgan decided the editor (M3) is built on the engine's own UI entities, and that the UI should recreate the important parts of HTML, with Godot's UI as a second reference. A survey of the code found 30-odd HTML elements with no equivalent (image, canvas/svg, embedded viewport, video/audio, select, menus, radio, tooltips, textarea, number/password/color/file/date inputs, form, table/grid, lists, tree, details, tabs, split panes, slots, semantic roles, links, rich text, headings/fonts), plus CSS gaps. This plan says how each one is built, in what order, and what must be decided first.

## What the code gives us today (verified)
- `UiDraw` has only `Rect` and `Text` (`crates/sage_core/src/host.rs`). The sprite batch already draws textured quads from any `TextureId` (`sprite.rs`, `GpuRenderer::add_texture`), so images need a new draw command and an asset-to-texture path, not a new renderer.
- Quads are axis-aligned only; the shaders are hand-assembled SPIR-V (`tools/shaderc`) with no uniforms. Rounded corners, rotation, gradients and shadows cannot be shaders without extending that tool.
- Text is one 5x7 upper-case bitmap face; its metrics are constants in `sage_core::host` that layout reads.
- Layout is one flow (row/column, fill weights, absolute children), no margin, grid, wrap or min/max.
- Input: one pointer button, wheel, one typed character per tick, `NAV_*` bits with no modifiers. `sage-input v1` records button bits only.
- One window (`ui_window`), GPU only. Audio mixer exists, WAV only.
- No cached crates for fonts, shaping, audio decoding, SVG or accessibility; every one of those is a new dependency, which needs Morgan's approval.

## Decisions (Morgan, 2026-11-11: all six taken as recommended)
The numbered items below were the questions; the recommendation in each is now the decision. Dependencies named in 1 and 4 are still **not approved**: they are asked about when their phase starts.

1. **Fonts:** keep bitmap fonts only (several atlases, lower case, Latin-1) or add a TrueType rasterizer (`fontdue` or `ab_glyph`, MIT/Apache) and optionally shaping (`rustybuzz`). Recommendation: a `Font` trait now, bitmap implementation first, TrueType behind it after approval.
2. **Shader work:** extend `tools/shaderc` for rounded rectangles (SDF), rotation and group opacity, or stay with textures: generated 9-slice corner and shadow images. Recommendation: textures first (no toolchain risk), shader later if profiling or looks demand it.
3. **Pointer recording:** `sage-input v1` cannot replay the mouse. Recommendation: a v2 with a pointer stream and modifiers, with v1 still readable.
4. **Dependencies:** audio decoders (OGG, MP3), SVG parsing, accessibility (`accesskit`), a Unicode segmentation crate for text. Each is optional and separable.
5. **Video:** recommendation is not to build video decoding (no usable small dependency); ship an image-sequence / sprite-sheet player first and revisit.
6. **Multi-window:** needed only for editor tab tear-off; defer until the editor needs it.

## Foundations (each is shared by many elements)
- **F1 Draw model.** Add `UiDraw::Image { asset, uv, pos, size, tint, slice: Option<[f32;4]> }` (plain image, sub-rect, 9-slice) and `Triangles` (arbitrary triangles with vertex colors, for gradients, vector paths and rotated quads). Per-draw alpha multiply gives opacity; transforms become a node component that rotates/scales the emitted quads. Extend `SpriteBatch`, `compose_ui`, `GpuRenderer` and the CPU `rasterize` reference together, so every change stays testable without a GPU.
- **F2 Assets in the UI.** Components hold an asset id (`u64`, the hash `AssetStore` already makes); presentation owns the id-to-texture table. Needs a PNG load path in `ui_window` and the `sage_assets` store wired in; no change to determinism.
- **F3 Layers.** A root can be an overlay with its own z, an anchor (a node or a point), a placement with flip-on-overflow, "dismiss on outside click or Escape", and a focus stack that restores focus on close. Popups, dropdowns, menus, tooltips, dialogs and pickers all use it. Replaces the playground's hand-built modal.
- **F4 Text engine.** A pure `text` module: font trait, spans (style runs), line breaking and measuring, a line buffer with selection ranges, word and line motion, per-field undo. Clipboard and IME through SDL (`SDL_GetClipboardText`; editing events from text input). Layout reads metrics through the font trait, not constants.
- **F5 Layout.** Margin, min/max size, percent and aspect-ratio sizes, `align-self`, wrap, grid (explicit tracks: fixed, fill, auto), relative and sticky position, scroll on both axes. Table is grid plus column sizing.
- **F6 Style.** Style variants selected by state (`hover`, `focus`, `pressed`, `disabled`, `checked`): a variant is a prefab-like component set applied while the state is on, replacing the hardcoded tints in `draw.rs`. Transitions and animations are deterministic tick-based tweens stored in components. Rounded corners, borders, shadows and gradients come from F1.
- **F7 Interaction.** Pointer capture, drag source and target events, drag-to-select text, resize handles, right and middle buttons, keyboard modifiers and shortcuts, focus-visible, touch. Needs decision 3 to be replayable.
- **F8 Semantics.** A `Role` and `AccessibleName` component on nodes, `label for` as a relationship, a derived accessibility tree for `accesskit` (decision 4).
- **F9 Render-to-texture and viewports.** The GPU renderer renders a `Frame` or a sub-UI into an offscreen texture (already done for screenshots in `render_to_image`), shown as an image; pointer and keyboard are forwarded to the embedded simulation.

## Every element, how it is built
| HTML / Godot | Built from | New pieces |
|---|---|---|
| `<img>`, `<picture>` | F1, F2 | `Image` component (asset, fit: contain/cover/stretch/none, tint); `<picture>` = choose by scale or size |
| 9-slice, icons, rounded corners, shadows | F1, F6 | `Slice` on `Paint`; generated corner and shadow textures |
| `<canvas>` | F1 | `Canvas` node holding a list of draw commands a Python system writes each tick (rects, lines, triangles, text, images) |
| `<svg>` | F1, F4 | path subset (move, line, cubic, arc, fill, stroke) tessellated to `Triangles`; a parser dependency only if decision 4 allows |
| `<iframe>`, `SubViewportContainer` | F9 | `Viewport` widget with an embedded `Frame` source and input forwarding |
| `<video>` | F1, F2 | image-sequence player first; decoding is decision 5 |
| `<audio>`, media controls | existing mixer | `AudioPlayer` widget: play, pause, seek, volume over `sage_platform::mixer`; formats by decision 4 |
| `<select>`, `<option>`, `<datalist>`, `OptionButton` | F3, F4 | `Dropdown` (popup list), `ComboBox` (text input plus filtered list); `Option` children |
| `<menu>`, `MenuBar`, `PopupMenu`, context menu | F3, F7 | `MenuBar`, `Menu`, `MenuItem` (check, radio, separator, submenu, shortcut text); right-click opens at the pointer |
| `<input type=radio>` | F6 | `RadioGroup` relationship; exclusive `checked` |
| tooltips (`title`, `<abbr>`) | F3 | `Tooltip` text on a node; delay, placement, dismiss |
| `<textarea>`, `TextEdit` | F4, F7 | `TextArea`: multi-line, selection, clipboard, undo, scroll, wrap, tab handling; also the Python editor (M3) |
| `<input type=text>` completion | F4 | selection, clipboard, placeholder, max length, horizontal scroll, blinking caret |
| `<input type=number>`, `SpinBox` | F4, F7 | `NumberField`: parse, clamp, step, drag-to-change, wheel |
| `<input type=password>` | F4 | `masked` flag; no clipboard copy |
| `<input type=color>`, `ColorPicker` | F1, F3, F7 | `ColorPicker` popup (HSV area, hue slider, alpha, hex field) built from the other widgets |
| `<input type=file>`, file dialogs | SDL native dialogs (`SDL_ShowOpenFileDialog`), F3 | `FileField`; an in-engine file browser (tree plus list) for platforms without a native dialog |
| `<input type=date/time>` | F3, F5 | `DateField` with a calendar grid popup; own calendar math (no `chrono`) |
| `<form>`, `<fieldset>`, `<legend>` | F8 | `Form` (group, submit and reset events, validation: required, min, max, pattern, custom), `Fieldset` with a legend |
| `<label for>` | F8 | `LabelFor` relationship: click focuses or toggles the control |
| `<table>`, `<tr>`, `<td>`, `<thead>`, Godot `GridContainer` | F5 | `Grid` (tracks, gaps, spans), `Table` (header row, column widths, sort, selection) |
| `<ul>`, `<ol>`, `<li>`, `ItemList` | F5, F6 | `List` with markers (bullet, number), selection modes, a virtualized variant for long lists |
| tree view, Godot `Tree` | F5, F6, F7 | `Tree`: expand/collapse, selection, drag-reorder, columns, keyboard navigation |
| `<details>`, `<summary>` | F6 | `Details` (open component, animated height) |
| tabs, `TabContainer` | F6, F7 | `Tabs`: bar, panels, close buttons, reorder by drag, overflow menu |
| split panes, `SplitContainer` | F7 | `Split`: draggable divider, min sizes, collapse |
| `<template>`, `<slot>` | prefabs | `Slot` marker on a prefab child: instantiation fills it with the caller's children |
| `<hr>`, `<nav>`, `<header>`, `<footer>`, `<main>`, `<section>` | F8 | `Role` values plus `Separator` and prefabs (no new layout) |
| `<a>` | F4, F7 | `Link` span in rich text and a `LinkButton`; click emits `Navigate` with a target |
| `<strong>`, `<em>`, `<code>`, `<span>`, `RichTextLabel` | F4 | rich text: spans with color, weight, italic, code font, links, inline images; a small markup form and a Python builder |
| `<h1>`-`<h6>`, `<p>`, font families | F4, theme | `Heading(level)` and `Paragraph` prefabs over named font sizes; font families through decision 1 |
| partial: range, progress, checkbox, dialog | existing | vertical and two-thumb slider; indeterminate meter and checkbox; `Dialog` as an F3 layer with top-layer behavior, automatic centering and focus return |
| CSS: grid, wrap, margin, min/max, percent, sticky | F5 | listed in F5 |
| CSS: shadows, gradients, rounded, opacity, transforms, transitions, animation | F1, F6 | listed in F1 and F6 |
| drag and drop, selection, resize | F7 | `Draggable`, `DropTarget`, `Resizable` components and events |

## Order of work (each phase ships tested, and adds a page to the playground)
- **P0** Decisions above. *Done 2026-11-11:* design doc revision 8, new 9.5, 10.6 decided.
- **P1** F1 + F2 + F6 (variants, rounded and shadow by texture): image, 9-slice, icons, opacity, gradients, borders; playground page "Media and style".
- **P2** F3 layers: tooltips, dropdown/select/combo/datalist, popup menus, menu bar, context menu, dialog rebuilt on layers.
- **P3** F4 text engine and F7 selection/clipboard: text input completed, textarea, number, password, placeholder, rich text, headings, links, fonts (per decision 1).
- **P4** F5 layout: margin, min/max, percent, wrap, grid, table, lists.
- **P5** Containers: tabs, split, details, tree, list virtualization, radio, label-for, form, slots.
- **P6** F7 drag and drop, resize, modifiers and shortcuts, right and middle buttons; pointer recording (decision 3).
- **P7** F9 viewport embed; multi-window tear-off if needed; canvas.
- **P8** F8 semantics and accessibility.
- **P9** Pickers (color, file, date/time), audio player, svg, video per decision 5.

Items needed first by the editor (M3) are P1-P5 and the viewport from P7: image, popups, textarea, tabs, split, tree, grid.

## Rules every phase follows (from CLAUDE.md)
- Pure logic (layout, text, interaction, draw) stays in dependency-free modules with unit tests; the ECS module only reads and writes components.
- All UI state is in components so it is hashed and saved; no mutable Python variables.
- New components under `sage.ui` regenerate `python/sage/ui.pyi` (`UPDATE_STUBS=1 cargo test -p sage_ui --test stubs`); schema changes to existing components are called out.
- Any new dependency or file format change is asked about first and recorded in `docs/dependencies.md` and the design doc.
- Each phase: `cargo test --workspace` in debug and release, clippy `-D warnings`, fmt, a playground page with screenshots checked by eye, `docs/milestone-3.md` status and a design doc note (9.x).

## Critical files
`crates/sage_ui/src/{layout,interact,draw,ecs}.rs`, `crates/sage_ui/scripts/widgets.py`, `crates/sage_core/src/host.rs`, `crates/sage_platform/src/{sprite,gpu,ui_window}.rs`, `crates/sage_assets/`, `tools/shaderc` (only if decision 2 says so), `examples/ui_playground/`, `claude/game-engine-design-doc.md`.

## Verification of the plan itself
Each table row becomes a check in `docs/milestone-3.md` with the test that proves it; the playground gains one page per phase, so the whole HTML-parity surface is visible and tested in one place. Real-window behavior (mouse, clipboard, IME, native file dialogs) is verified by hand with `ui_playground play` and `smoke`, since CI has no display.
