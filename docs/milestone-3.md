# Milestone 3: UI parity status

Date: 2026-11-11. Scope: the ECS UI that the editor (design doc 10.6) is built on, as planned in `docs/ui-html-parity.md` (design doc 9.5). M3 itself is the editor; this file tracks the UI phases P1 to P9 that come first, each with the test that proves it. Real-window behavior (mouse, clipboard, IME, native dialogs) is checked by hand with `cargo run -p ui_playground -- play`, because CI has no display.

No dependency has been added. Still **not approved** and not used: a TrueType rasterizer (`fontdue`, `ab_glyph`), `rustybuzz`, audio decoders, an SVG parser, `accesskit`, a Unicode segmentation crate. Each is asked about, with its license, when its phase needs it.

## Phase status

| Phase | Scope | Status |
|---|---|---|
| P1 | images, 9-slice, rounded corners, shadows, gradients, fade, `display`, state variants, transitions, toggle button | Done (`73c0eee`) |
| P2 | layers: tooltips, select, combo, menus, menu bar, context menu, dialog | Done |
| P3 | text engine: selection, clipboard, undo, text area, password, number, placeholder, rich text, headings, links | Done, with the gaps below |
| P4 | layout: margin, min/max, percent, aspect, align-self, offset, wrap, grid, table, lists | Done, with the gaps below |
| P5 | tabs, split, details, tree, radio, label-for, form, slots | Not started |
| P6 | drag and drop, resize, modifiers, shortcuts, right and middle buttons, `sage-input v2` | Not started (the file format layout is shown to Morgan first) |
| P7 | viewport embed, canvas | Not started |
| P8 | semantics and accessibility | Not started (needs `accesskit` approval) |
| P9 | color, file and date pickers, audio player, SVG, video | Not started (decoders and SVG need approval) |

## Checks

| Element (HTML / Godot) | Built as | Proved by |
|---|---|---|
| `<img>`, 9-slice, icons, rounded corners, shadows, gradients, opacity, `display: none`, hover/press/focus/disabled/checked variants, transitions | P1 components and draw variants | `sage_ui` tests; `playground.rs` STYLE tests |
| `<select>`, `<option>` | `ui.select`, `Select`, `Option` | `a_select_opens_a_list_picks_an_option_and_closes` |
| `<datalist>`, combo box | `ui.combo` | `a_combo_box_narrows_its_list_as_you_type_and_picks_one` |
| menu bar, submenus, check items, shortcut text | `ui.menu_bar`, `ui.menu` | `the_menu_bar_opens_submenus_and_chooses_items` |
| context menu | `ui.context_menu` | `a_right_click_opens_the_context_menu_at_the_pointer` |
| tooltips | `ui.tooltip` | `a_tooltip_appears_after_its_delay_and_goes_when_the_pointer_leaves` |
| `<dialog>` | `ui.dialog` on a modal layer, focus returns | `the_dialog_is_a_modal`, `the_confirm_dialog_returns_focus_to_its_button_and_escape_cancels` |
| `<input>` selection, clipboard, undo, word motion | `sage_ui::text`, `TextField`, `History` | `text.rs` and `interact.rs` unit tests; `selection_replaces_and_the_clipboard_round_trips_to_the_window`, `dragging_the_mouse_selects_text` |
| `placeholder`, `maxlength` | `TextField` | `a_field_shows_its_placeholder_until_typed_in_and_counts_what_is_typed` |
| `<input type=password>` | `TextField` flag 2 | `a_password_shows_asterisks_and_will_not_copy` |
| `<textarea>` | `TextField` flag 1: wrap, Enter, Up/Down, wheel, scroll | `a_text_area_wraps_takes_enter_and_moves_by_row` |
| `<input type=number>` | `TextField` flag 4 plus `Slider` for value, range, step | `number_fields_clamp_snap_step_and_report` |
| `<strong>`, `<code>`, colors, `<a>` | `Rich` markup; `Navigate` event | `rich_text_draws_styled_runs_and_a_link_click_navigates` |
| `<h1>` to `<h3>`, `<p>` | `ui.heading`, `ui.rich` | same |
| margin, min/max, percent, aspect-ratio, align-self, relative position | `Space`, `Size::Percent` | `layout.rs` unit tests; `margins_limits_percentages_ratio_and_offsets_place_the_boxes` |
| `flex-wrap` | `WrapFlow` | `a_wrapping_row_starts_a_new_line...`, `a_wrapping_row_breaks_into_lines_and_the_grid_spans_cells` |
| `display: grid`, `GridContainer` | `Grid`, `Cell` | `a_grid_sizes_fixed_auto_and_fractional_tracks`, `grid_children_span_tracks_and_may_be_placed_by_hand` |
| `<table>`, `<th>`, `<td>` | `ui.table`: grid with sortable header and selectable rows | `a_table_sorts_by_a_header_and_selects_a_row` |
| `<ul>`, `<ol>`, `<li>` | `ui.list` | `lists_have_markers_and_wrapping_items` |

## Decisions made while building
1. **Only new components.** `UiCaret` was left alone; selection and scrolling live in the new `TextField`, undo in `History`, spacing in `Space`, grids in `Grid` and `Cell`. No schema of an existing component changed.
2. **NAV bits carry text commands.** `NAV_SHIFT` and `NAV_WORD` are held modifiers; `NAV_SELECT_ALL`, `COPY`, `CUT`, `PASTE`, `UNDO`, `REDO` are edges the window sets from Ctrl+A, C, X, V, Z, Y. This is not the `sage-input` file format (v2 is P6).
3. **Clipboard goes through a singleton.** `Clipboard { text, out }`: a copy sets it and `out`; the window publishes it to the system; a paste reads it, and the window fills it from the system clipboard on the tick a paste is asked for.
4. **A layer under a modal remembers the focus from before the modal dropped it**, so closing a dialog returns focus to its button.
5. **Table state is in components** (`Table`, `HeadOf`, `CellOf`), so a save game restores the sort and selection.

## Gaps
- **Fonts:** bitmap only. Bold is the glyphs drawn twice one unit apart; there is no italic, no other family, and the glyph set is the built-in face (no `@` or `*`). A TrueType crate needs approval.
- **Text:** no IME composition (SDL editing events), no double-click word selection, no per-field horizontal and vertical scroll bars, and a link fires on release over it without remembering where the press began.
- **Layout:** no sticky position; scrolling is along the flow direction only (no two-axis scroll); `min`/`max` clamping after sharing out `fill` space does not redistribute the remainder; wrapping column flows need a fixed height. A list is not virtualized yet.
- **Table:** no column resize, no reorder; sorting is by the displayed text (numbers sort as numbers).
- **Not in the playground yet:** P5 to P9 elements.
