# Milestone 3: UI parity status

Date: 2026-11-11. Scope: the ECS UI that the editor (design doc 10.6) is built on, as planned in `docs/ui-html-parity.md` (design doc 9.5). M3 itself is the editor; this file tracks the UI phases P1 to P9 that come first, each with the test that proves it. Real-window behavior (mouse, clipboard, IME, native dialogs) is checked by hand with `cargo run -p ui_playground -- play`, because CI has no display.

Morgan approved `accesskit`, `fontdue`, OGG and MP3 decoders, an SVG parser, `rustybuzz`, `unicode-segmentation` and the Windows `accesskit` adapter on 2026-11-11 (see `docs/dependencies.md`); they are in. Still **not approved**: the Linux and macOS `accesskit` adapters.

## Phase status

| Phase | Scope | Status |
|---|---|---|
| P1 | images, 9-slice, rounded corners, shadows, gradients, fade, `display`, state variants, transitions, toggle button | Done (`73c0eee`) |
| P2 | layers: tooltips, select, combo, menus, menu bar, context menu, dialog | Done |
| P3 | text engine: selection, clipboard, undo, text area, password, number, placeholder, rich text, headings, links | Done, with the gaps below |
| P4 | layout: margin, min/max, percent, aspect, align-self, offset, wrap, grid, table, lists | Done, with the gaps below |
| P5 | tabs, split, details, tree, radio, label-for, form, slots | Done, with the gaps below |
| P6 | drag and drop, resize, modifiers, shortcuts, right and middle buttons, `sage-input v2` | Done, with the gaps below |
| P7 | viewport embed, canvas | Done, with the gaps below (multi-window tear-off deferred, as decided) |
| P8 | semantics and accessibility | Done; the Windows adapter is written but unrun, Linux and macOS have none |
| P9 | color, file and date pickers, audio player, SVG, video | Done (native dialogs unverified on a screen; video is an image sequence, as decided) |

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
| tabs, `TabContainer`, close buttons | `ui.tabs` (`Tabs`, `TabOf`, `TabClosed`) | `tabs_show_one_panel_and_a_close_button_hides_a_tab` |
| split panes, `SplitContainer` | `ui.split`: `Handle` widget, `Drag`/`Dragged`, min sizes, collapse by `split_set` | `dragging_the_divider_resizes_the_panes_within_their_minimums`; `a_handle_reports_the_drag...` (interact.rs) |
| `<details>`, `<summary>` | `ui.details` | `details_open_and_close_with_their_header` |
| tree view, Godot `Tree` | `ui.tree` (`TreeRow`, `ArrowOf`, `TreeSelected`) | `a_tree_expands_branches_and_selects_one_row` |
| `<input type=radio>` | `ui.radio_group` | `radio_buttons_are_exclusive_and_a_label_flips_its_control` |
| `<label for>` | `Label` widget plus `LabelFor` | same |
| `<form>`, validation | `ui.form`, `form_row`, `form_submit`; `FormSubmitted`, `FormInvalid` | `a_form_validates_shows_errors_and_submits_or_resets` |
| `<template>`, `<slot>` | `ui.slot`, `ui.instantiate` | `a_template_puts_the_callers_children_in_its_slots` |
| P5 state in a save game | all of it is components | `the_panels_page_survives_a_save_game` |
| drag and drop | `Draggable`, `DropZone`; `DragStarted`, `Dropped`, `DragEnded`; `Dnd` state singleton | `a_draggable_picks_up_after_a_few_units...`, `releasing_a_drag_off_every_zone...` (interact.rs); `a_drop_zone_takes_only_the_kinds_it_accepts...` |
| reorderable list | `ui.sortable` over drag and drop | `dragging_a_row_onto_another_reorders_the_list` |
| resize handles | `ui.resizable` (`ResizeOf`, a `Handle` grip) | `the_grip_resizes_a_panel_down_to_its_minimum` |
| keyboard shortcuts | `Shortcut`, `ui.shortcut("Ctrl+S")`; window chords | `a_shortcut_chord_clicks_its_node...` (interact.rs); `key_chords_click_their_widget...`; `the_middle_button_and_key_chords_reach_the_tick_once` (ui_window.rs) |
| modifiers | `NAV_SHIFT`, `NAV_WORD` (Ctrl), new `NAV_ALT`; chords carry `CHORD_CTRL/SHIFT/ALT` | same |
| middle and right buttons | `MiddleClicked`; `ContextRequested` (P2) | `the_middle_button_clicks_on_release...` (interact.rs); `key_chords_click_their_widget_and_a_middle_click_is_its_own_event` |
| `<canvas>`, vector shapes | `ui.canvas`, `draw_line`/`draw_triangle`/`draw_ellipse`/`draw_rect` over `Shape`; new `UiDraw::Triangle` and `Quad` | `a_canvas_draws_its_shapes_and_cuts_them_off_at_its_box`, `a_script_redraws_a_canvas_every_tick_without_making_new_entities`; `a_polygon_is_cut_to_a_rectangle...`, `a_line_is_a_thick_quad...` (draw.rs); `triangles_and_free_quads_fill_only_what_they_cover` (sprite.rs) |
| `<iframe>`, `SubViewportContainer` | `ui.viewport` (`Viewport`, `Widget::Viewport`); the host gives frames with `sage_ui::set_viewport_frame` and reads the pointer back | `a_viewport_shows_the_hosts_simulation_and_forwards_the_pointer_to_it`; `a_viewport_letterboxes_the_world_and_draws_it_y_up`; `a_viewport_reports_the_pointer_over_it_and_while_dragging_from_it` |
| P7 state in a save game | shapes and the viewport component are components | `the_canvas_page_survives_a_save_game` |
| roles, names, `aria-*` | `Access` component, `ui.role`; `sage_ui::access` derives the tree (`access_tree`, `access_outline`); `ui_playground access --page NAME` prints it | `access.rs` unit tests; `the_python_role_list_is_the_rust_one`, `controls_are_named_by_their_labels_and_report_their_state` |
| `<nav>`, `<header>`, `<footer>`, `<main>`, `<section>`, `<hr>`, `<fieldset>` | `ui.nav`, `header`, `footer`, `main`, `section`, `separator`, `fieldset`: panels with a role, no new layout | `landmarks_headings_groups_labels_and_alt_text_reach_the_tree` |
| `<label for>` as a relationship, `aria-labelledby` | `Access.label_for` and `labelled_by`; the kit's `ui.label_for` and `ui.toggle(text=)` set them | same |
| alt text, decoration | `ui.image(..., alt=)`; `alt=""` leaves it out | same |
| headings, widgets' implied roles | `ui.heading` sets the role and level; buttons, sliders, text boxes, meters, toggles imply theirs | same |
| passwords, disabled, focus, open popups | the tree hides a password's text, reports `[disabled]` and `[focused]`, and holds a popup only while it is open | `a_password_never_exposes_its_text_and_a_disabled_button_says_so`, `popups_are_in_the_tree_only_while_they_are_open` |
| `<input type=color>`, `ColorPicker` | `ui.color_field`: area grip, hue and alpha sliders, hex field; `ColorChanged` | `a_color_is_picked_by_dragging_the_square_the_sliders_or_typing_hex` |
| `<input type=date>` | `ui.date_field`: calendar popup, own calendar math; `DatePicked` | `calendar_math_is_right_for_known_dates`, `a_calendar_opens_pages_through_months_and_picks_a_day` |
| `<input type=time>` | `ui.time_field`: two clamped number fields; `TimePicked` | `a_time_field_clamps_each_part_and_reports` |
| `<input type=file>` (in-engine browser) | `ui.file_field` over `ui.tree`; `ui.file_tree(paths)`; `FilePicked` | `a_file_is_picked_from_a_tree_of_paths` |
| `<audio>`, media controls | `ui.audio_player` (`Media`, `MediaParts`); `sage_platform::media::MediaVoice` binds it to the mixer (new pause, seek, volume, position) | `media_controls_play_pause_stop_at_the_end...`, `the_scrubber_and_volume_slider_move_the_player`, `the_host_plays_the_sound_the_player_says...`; `media.rs` and `mixer.rs` tests |
| `<video>` | `ui.video_player`: an image sequence under media controls | `a_video_shows_the_frame_for_its_time_and_loops` |
| `<svg>` | `ui.svg`, `ui.svg_path` over a canvas; path data, basic shapes, fill and stroke | `svg_draws_paths_and_shapes_and_the_helpers_are_exact` |
| P9 state in a save game | components | `the_pickers_page_survives_a_save_game`, `the_media_page_survives_a_save_game_and_shows_in_the_access_tree` |
| sticky, two-way scroll, `order` | `Sticky`, `ScrollCross`, `Order`; `ui.scroll(both=True)`, `add(sticky=, order=)` | `a_sticky_row_stays_at_the_top...`, `a_container_can_scroll_across_its_flow_too`, `order_keys_put_siblings...` (layout.rs); `a_sticky_header_holds_while_the_list_scrolls...` |
| tabs and tree reorder by drag | `reorder=True` over `ui.reorderable` (`Order` keys, `Moved`) | `tabs_and_tree_rows_are_dragged_into_a_new_order...` |
| virtualized list | `ui.virtual_list` | `a_virtual_list_of_a_hundred_thousand_items_is_six_rows` |
| double and triple click, link press origin, IME pre-edit | `LastPress`, `press_link`, `Composition` | `a_double_click_selects_a_word...` (interact.rs), `double_and_triple_clicks_select...`, `an_input_methods_pre_edit_text_is_drawn...`, `composing_text_shows_until_it_is_committed` |
| SVG documents | `sage_ui::svg` over `roxmltree`; `ui.svg` | `svg.rs` tests; `an_svg_document_draws_its_shapes_and_a_bad_one_shows_magenta` |
| accesskit update | `sage_ui::access_kit`, `access_update` | `a_tree_maps_roles_names_states_and_children`, `the_accesskit_update_of_a_real_page_is_a_consistent_tree` |
| Ogg and MP3 | `sage_assets::compressed` (`decode_any`); the asset store reads `.ogg` and `.mp3` | `compressed.rs` and `store.rs` tests |
| TrueType text | `BitmapFont::from_ttf` (`fontdue`); `ui_playground --font FILE.ttf` | `font.rs` ttf tests; `a_truetype_face_draws_lower_case...` (skipped without a system font) |
| native file dialog | `ui.file_field(native=True)`, `answer_file_dialog` | `a_native_file_field_asks_the_host_for_its_dialog_and_takes_the_answer` |
| proportional shaped text | `from_ttf_proportional`, `ShapedFace`, `TextMetrics`, `node_text_size`/`node_wrap_text` | `a_shaped_face_gives_each_glyph_its_own_advance_and_kerning`; `a_proportional_face_sizes_labels_by_their_letters_and_leaves_inputs_on_the_grid` |
| grapheme caret | `prev_grapheme`, `next_grapheme` | `the_caret_and_delete_move_by_grapheme_not_by_code_point` |
| table column resize, details height | `ColGrip`, `ui.table_set_column`; `Reveal` | `dragging_a_header_grip_gives_a_table_column_a_width`, `details_open_and_close_with_their_header`, `reveal_shows_a_fraction...` |
| assistive actions | `perform_access_action`, `UiApp::access_action`, `sage_platform::a11y` | `assistive_technology_can_focus_and_press_controls_like_the_pointer_would` |
| bold and italic | `STYLE_BOLD`/`STYLE_ITALIC`, `BitmapFont::layout_styled`, `*italic*` markup | `bold_and_italic_change_the_glyphs...`, `single_asterisks_make_italics_in_markup...`, `rich_text_draws_styled_runs...` |
| `sage-input v2` | `PointerInput`, `InputRecording::record_pointer`; `ui_playground play --record`, `replay` | `sage_core` input tests; `a_recorded_mouse_session_replays_to_the_same_state_every_tick` |

## Decisions made while building
1. **Only new components.** `UiCaret` was left alone; selection and scrolling live in the new `TextField`, undo in `History`, spacing in `Space`, grids in `Grid` and `Cell`. No schema of an existing component changed.
2. **NAV bits carry text commands.** `NAV_SHIFT` and `NAV_WORD` are held modifiers; `NAV_SELECT_ALL`, `COPY`, `CUT`, `PASTE`, `UNDO`, `REDO` are edges the window sets from Ctrl+A, C, X, V, Z, Y. This is not the `sage-input` file format (v2 is P6).
3. **Clipboard goes through a singleton.** `Clipboard { text, out }`: a copy sets it and `out`; the window publishes it to the system; a paste reads it, and the window fills it from the system clipboard on the tick a paste is asked for.
4. **A layer under a modal remembers the focus from before the modal dropped it**, so closing a dialog returns focus to its button.
5. **Table state is in components** (`Table`, `HeadOf`, `CellOf`), so a save game restores the sort and selection.

## Gaps
- **Fonts:** `--font FILE.ttf` draws with a monospaced TrueType face on the fixed grid; with `--proportional` static text (labels, buttons, paragraphs) is shaped by `rustybuzz`, measured and wrapped with the face's own advances and kerning, and drawn from a glyph-id atlas (`BitmapFont::from_ttf_proportional`, `sage_core::host::TextMetrics`, installed per thread). Still on the grid by design: text inputs (the caret and selection count columns), rich text (bold and links are placed by column), and the atlas holds Latin through Latin Extended-B, Greek and Cyrillic (other scripts, and ligature glyphs outside it, draw `?`). Bold and italic are a `style` on `UiDraw::Text` (`STYLE_BOLD`, `STYLE_ITALIC`): a TrueType face has them baked into its atlas (bold struck wider, italic slanted about the baseline, from the one regular face: synthetic, not separate bold or italic font files), and the built-in face strikes bold twice and has no italic. Rich text takes `**bold**` and `*italic*`. No true bold or italic font files and no per-node families.
- **Text:** the caret and delete move by grapheme cluster (`unicode-segmentation`); no per-field scroll bars; the IME pre-edit text is drawn at the caret but not placed in the candidate window (no `SDL_SetTextInputArea`) and is not recorded in `sage-input`; static text (not an input) cannot be drag-selected.
- **Layout:** sticky applies to direct children of a scroll container along its flow; `min`/`max` clamping after sharing out `fill` space does not redistribute the remainder; wrapping column flows need a fixed height. 
- **Table:** no column resize, no reorder.
- **Containers:** the tree has no columns and no arrow-key navigation beyond Tab; tabs have no overflow menu; table columns resize by a grip (`ui.table(resizable=True)`) but do not reorder; `<details>` animate their height over 8 ticks through `Reveal` (auto-height only). Tabs and tree rows reorder by drag (`reorder=True`), a list of any length is virtualized (`ui.virtual_list`).
- **Forms:** a number field clamps itself, so its range error never shows; `form_row` has no `min`/`max` for text length beyond `min_len` and `max_len`.
- **Drag and drop:** no drag ghost is drawn while dragging (the drop zone shows a hint through `DropZone.over`); nothing reparents entities, so a drop is reported and the game decides what moves; tabs and tree rows are not yet reorderable by drag (the pieces exist: `ui.draggable` and `ui.drop_zone` on them); a drag does not scroll a container at its edge.
- **Input:** the recorded `sage-input v2` keeps the clipboard text out (a paste on replay reads the replaying machine's clipboard); touch and pen are not handled; no key-up or per-key state is recorded beyond the `NAV_*` bits and chords, so a game that needs raw keys will need another stream.
- **`sage-input v2`:** as approved by Morgan on 2026-11-11, except one correction to the proposal's wording: as in v1, a tick with no line is *empty*, not a repeat of the previous tick (v1 never carried state over, and v2 keeps that rule).
- **Canvas:** shapes are lines, triangles, ellipses and rectangles, filled and flat-colored (no strokes on shapes other than lines, no curves, no text or images on a canvas beyond what ordinary nodes give); every shape is an entity, which is fine for hundreds, not for tens of thousands. Drawing animation uses no trigonometry on purpose (platform `libm` results are not guaranteed identical, and shape floats are hashed).
- **Viewport:** the host draws the embedded simulation's `Frame` rectangles directly into the UI draw list, not into an offscreen texture, so it is limited to what `Frame` carries (rectangles by `DrawKind`, a HUD). A richer embedded renderer needs the offscreen texture path (F9 proper). The embedded simulation, its input and its saves belong to the host; the UI only draws and forwards the pointer (and Tab and keys already reach a focused viewport through the usual input). Multi-window tear-off is deferred until the editor needs it.
- **Playground:** the screen is now 480x300 units (it was 480x270), because the page switcher wraps to two rows with this many pages.
- **Accessibility:** `sage_ui::access_update` makes the `accesskit::TreeUpdate`, `perform_access_action` takes focus and click from assistive technology (both tested headless), and `sage_platform::a11y` connects them to the window through the Windows UI Automation adapter. The whole `sage_platform::a11y` module, SDL window-handle glue included, compiles for `x86_64-pc-windows-gnu` (checked as a standalone crate against the real `sdl3` and `accesskit_windows`), but it was never run: this machine is Linux. No Linux or macOS adapter exists (not approved), and no real screen reader has been tried anywhere. Not done: a live-region role and announcements, text selection and caret ranges in the tree, individual links inside a rich-text paragraph (the paragraph is one `text` node), the `expanded` and `selected` states beyond a toggle's on/off, keyboard-only access to a drag, and `aria-describedby`.
- **Pickers:** the color picker has no eyedropper, no palette swatches and no non-RGB models; the calendar is the Gregorian one, has no week numbers, no range selection and no time zones; `ui.time_field` has no seconds and no AM/PM; the file browser lists paths the script gives it (the host builds that from the asset catalog or a directory), it does not read the disk itself, and it has no type filter, no multi-select and no "save as" name box. **Native file dialogs** are wired (`ui.file_field(native=True)`, `UiApp::take_file_request` and `file_chosen`, `sdl3::dialog` in the window loop) and the request and answer flow is tested headless. Run for real (`ui_playground play --pick-file` on a GNOME Wayland session) the window asked SDL for the dialog without an error and the GTK file-chooser portal backend was started on the session bus, but I could not screenshot or click the dialog from this environment, so choosing a file through it end to end has not been observed.
- **Media:** audio is WAV, Ogg Vorbis and MP3 (MPEG-1 only; see `docs/dependencies.md`). The playground window sends the mixer to the device through `UiApp::mix_audio`, which was written but only the mixing and binding are tested headless; hearing it needs `ui_playground play` on a machine with a sound device. There is no spatial audio, no playback rate control and no waveform view. A video is an image sequence: no decoding, no audio track sync, no subtitles.
- **SVG:** parsed by `roxmltree`; paths (all commands, arcs included), basic shapes, groups, transforms, fill, stroke and opacity. Not drawn: gradients, text, `<use>`, clipping, CSS selectors, line joins and caps, holes in fills; a fill is ear-clipped per subpath, so self-intersecting paths fill wrongly.
- **Playground:** its page names collide in the shared test namespace only if two pages reuse a top-level name; keep names unique per page.
- **Not in the playground yet:** nothing from the plan is missing a page, but the gaps above are what stands between each page and its HTML or Godot counterpart.
