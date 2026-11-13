//! The editor, driven the way a person does: clicks, drags and typing on
//! the UI, then looking at the document, the game and the source.

use editor::{Editor, sample_project};
use sage_core::host::{
    CHORD_ALT, CHORD_CTRL, CHORD_SHIFT, NAV_ACCEPT, NAV_CANCEL, NAV_SELECT_ALL, NAV_SHIFT,
};

fn editor() -> Editor {
    let ed = Editor::new(sample_project().unwrap(), None).unwrap();
    ed.idle(3);
    ed
}

fn field(ed: &Editor, comp: &str, f: &str) -> String {
    ed.eval(&format!(
        "[t for (p, n, fs) in editor.inspect()[2] if n == {comp:?} for (fn_, k, t) in fs if fn_ == {f:?}][0]"
    ))
}

fn select(ed: &Editor, sid: u64) {
    ed.click(&format!(
        "[c for c in outliner.children if text_of(c).strip().lower().endswith('#{sid}')][0]"
    ));
    ed.idle(2);
}

/// Focuses the inspector's number field, replaces its text, and commits.
fn type_into_field(ed: &Editor, comp: &str, f: &str, text: &str) {
    let e = format!("handles[('field', {comp:?}, {f:?})]");
    ed.click(&e);
    ed.press(NAV_SELECT_ALL);
    ed.type_text(text);
    ed.press(NAV_ACCEPT);
    ed.idle(3);
}

/// Everything on screen as text; wrapped lines are joined with spaces.
fn texts(ed: &Editor) -> String {
    ed.texts().join(" ").to_uppercase()
}

fn tab(ed: &Editor, i: usize) {
    ed.click(&format!("tab_buttons[{i}]"));
    ed.idle(3);
}

#[test]
fn the_outliner_lists_entities_and_clicking_one_fills_the_inspector() {
    let ed = editor();
    let t = texts(&ed);
    assert!(
        t.contains("HERO") || t.to_lowercase().contains("hero"),
        "{t}"
    );
    assert!(texts(&ed).contains("NOTHING SELECTED"));
    select(&ed, 1);
    assert_eq!(ed.eval::<Option<u64>>("editor.selection()"), Some(1));
    assert_eq!(field(&ed, "Position", "x"), "20.0");
    assert!(texts(&ed).contains("POSITION"));
    select(&ed, 2);
    assert_eq!(field(&ed, "Position", "x"), "90.0");
}

#[test]
fn typing_in_an_inspector_field_edits_the_document_and_undo_reverts_it() {
    let ed = editor();
    select(&ed, 1);
    type_into_field(&ed, "Position", "x", "42");
    assert_eq!(field(&ed, "Position", "x"), "42.0");
    assert!(ed.eval::<bool>("editor.dirty()"));
    assert!(texts(&ed).contains("UNSAVED"));
    ed.click("b_undo");
    ed.idle(3);
    assert_eq!(field(&ed, "Position", "x"), "20.0");
    ed.click("b_redo");
    ed.idle(3);
    assert_eq!(field(&ed, "Position", "x"), "42.0");
}

#[test]
fn the_document_reaches_the_live_world() {
    let ed = editor();
    select(&ed, 1);
    type_into_field(&ed, "Position", "x", "55");
    ed.idle(2);
    assert!(ed.project(|p| p.live_scene().write().contains("x=55.0")));
}

#[test]
fn removing_and_adding_components_from_the_inspector() {
    let ed = editor();
    select(&ed, 2);
    assert!(!texts(&ed).contains("VELOCITY"));
    ed.run("editor.add_component(2, [p for p, n in editor.addable() if n == 'Velocity'][0])");
    ed.idle(3);
    assert!(texts(&ed).contains("VELOCITY"));
    ed.run("editor.undo()");
    ed.idle(3);
    assert!(!texts(&ed).contains("VELOCITY"));
}

#[test]
fn add_copy_and_delete_buttons_work_on_the_selection() {
    let ed = editor();
    select(&ed, 3);
    let n = ed.eval::<usize>("len(editor.outliner())");
    ed.click("tools.children[1]"); // COPY
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n + 1);
    ed.click("tools.children[2]"); // DEL
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n);
    ed.click("b_undo");
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n + 1);
}

#[test]
fn play_runs_the_game_and_stop_puts_the_scene_back() {
    let ed = editor();
    let before = ed.project(|p| p.doc.scene.write());
    ed.click("b_play");
    ed.idle(30);
    assert!(texts(&ed).contains("PLAYING"));
    let live = ed.project(|p| p.live_scene().write());
    assert!(!live.contains("Position x=20.0"), "the hero moved: {live}");
    assert_eq!(ed.project(|p| p.doc.scene.write()), before);
    ed.click("b_play");
    ed.idle(3);
    assert!(texts(&ed).contains("EDITING"));
    assert!(
        ed.project(|p| p.live_scene().write())
            .contains("Position x=20.0")
    );
}

#[test]
fn dragging_in_the_scene_moves_the_entity_as_one_undo_step() {
    let ed = editor();
    let [x, y, w, h] = ed.rect("scene_view");
    let size: [f32; 2] = ed.eval("[160.0, 90.0]");
    // The viewport letterboxes a 160x90 world; find where the hero is.
    let scale = (w / size[0]).min(h / size[1]);
    let (ox, oy) = (
        x + (w - size[0] * scale) / 2.0,
        y + (h - size[1] * scale) / 2.0,
    );
    let at = |wx: f32, wy: f32| [ox + wx * scale, oy + (size[1] - wy) * scale];
    ed.drag(at(20.0, 30.0), at(70.0, 60.0));
    ed.idle(2);
    assert_eq!(ed.eval::<Option<u64>>("editor.selection()"), Some(1));
    let px: f32 = field(&ed, "Position", "x").parse().unwrap();
    let py: f32 = field(&ed, "Position", "y").parse().unwrap();
    assert!(
        (px - 70.0).abs() < 1.5 && (py - 60.0).abs() < 1.5,
        "{px} {py}"
    );
    ed.click("b_undo");
    ed.idle(3);
    assert_eq!(
        field(&ed, "Position", "x"),
        "20.0",
        "one undo reverts the whole drag"
    );
}

#[test]
fn the_code_tab_applies_new_rules_live_and_reports_errors() {
    let ed = editor();
    tab(&ed, 1);
    let shown: String = ed.eval("text_of(code_area)");
    assert!(shown.contains("def move(pos: Position, vel: Velocity)"));
    // A broken edit is reported and the old game keeps running.
    ed.run("code_area.set(Label(world.intern('def broken(:\\n')))");
    ed.click("fixed_click_button('APPLY')");
    ed.idle(3);
    assert!(texts(&ed).contains("ERROR"), "{}", texts(&ed));
    // A good one takes effect without a restart.
    let faster = shown.replace("vel.x * 0.1", "vel.x * 10");
    ed.run(&format!("code_area.set(Label(world.intern({faster:?})))"));
    ed.click("fixed_click_button('APPLY')");
    ed.idle(3);
    assert!(ed.eval::<Option<String>>("editor.error()").is_none());
    ed.click("b_step");
    ed.idle(2);
    let live = ed.project(|p| p.live_scene().write());
    assert!(live.contains("Position x=60.0"), "20 + 4*10: {live}");
}

#[test]
fn the_block_tab_shows_functions_as_stacks_with_comments_as_sticky_notes() {
    let ed = editor();
    tab(&ed, 2);
    let t = texts(&ed);
    for want in [
        "MOVES EVERYTHING THAT HAS A VELOCITY.",
        "ADD VELOCITY TO POSITION, A LITTLE EACH TICK.",
        "ONE TICK IS A SIXTIETH OF A SECOND",
        "WHEN",
        "MOVE",
        "CHANGE",
    ] {
        assert!(t.to_uppercase().contains(want), "{want}\n{t}");
    }
    // The palette comes from the reflected schema: this system's own
    // components read through their parameters.
    ed.run("blk['cat'] = 'game'; view_dirty[0] = True");
    ed.idle(3);
    let t = texts(&ed);
    assert!(t.contains("CHANGE POSITION'S X BY"), "{t}");
}

#[test]
fn clicking_a_palette_block_adds_it_and_the_text_follows() {
    let ed = editor();
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Control'; view_dirty[0] = True");
    ed.idle(3);
    let before = ed.eval::<String>("editor.source('game')");
    ed.click("handles[('palette', 'pass')]");
    ed.idle(4);
    let after = ed.eval::<String>("editor.source('game')");
    assert_ne!(before, after);
    assert!(
        after.contains("    pos.y += vel.y * 0.1\n    pass\n"),
        "{after}"
    );
    // Everything else is untouched, comments included.
    assert!(after.contains("# one tick is a sixtieth of a second"));
    assert_eq!(
        after,
        before.replace(
            "    pos.y += vel.y * 0.1\n",
            "    pos.y += vel.y * 0.1\n    pass\n"
        )
    );
    // The live game reloaded with it.
    assert!(ed.eval::<bool>("not editor.stale()"));
}

#[test]
fn block_buttons_delete_reorder_and_note() {
    let ed = editor();
    tab(&ed, 2);
    let before = ed.eval::<String>("editor.source('game')");
    // Move the second statement up: they swap.
    ed.click("handles[('stmt', 2, 'up')]");
    ed.idle(4);
    let swapped = ed.eval::<String>("editor.source('game')");
    assert!(
        swapped.find("pos.y +=").unwrap() < swapped.find("pos.x +=").unwrap(),
        "{swapped}"
    );
    // Add a note to the first statement, then delete it with its note.
    ed.click("handles[('stmt', 1, 'note')]");
    ed.idle(4);
    let noted = ed.eval::<String>("editor.source('game')");
    assert!(noted.contains("    # NOTE\n    pos.y +="), "{noted}");
    ed.click("handles[('stmt', 1, 'del')]");
    ed.idle(4);
    let gone = ed.eval::<String>("editor.source('game')");
    assert!(
        !gone.contains("NOTE") && !gone.contains("pos.y +="),
        "{gone}"
    );
    assert!(gone.contains("pos.x += vel.x * 0.1"));
    assert!(before.contains("pos.y += vel.y * 0.1"));
}

#[test]
fn typing_in_a_literal_block_edits_the_source_and_rejects_nonsense() {
    let ed = editor();
    tab(&ed, 2);
    // The first literal is `0.1` in `pos.x += vel.x * 0.1`.
    ed.click("handles[('lit', 1)]");
    ed.press(NAV_SELECT_ALL);
    ed.type_text("0.5");
    ed.press(NAV_ACCEPT);
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(
        src.contains("pos.x += vel.x * 0.5") && src.contains("pos.y += vel.y * 0.1"),
        "{src}"
    );
    // Something that is not a number changes nothing and says so.
    ed.click("handles[('lit', 1)]");
    ed.press(NAV_SELECT_ALL);
    ed.type_text("__import__('os')");
    ed.press(NAV_ACCEPT);
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), src);
}

#[test]
fn dragging_a_block_onto_another_moves_it_with_its_notes() {
    let ed = editor();
    tab(&ed, 2);
    let before = ed.eval::<String>("editor.source('game')");
    // The second statement is dropped on the first: they swap.
    let heads = "[h for h in dnd if dnd[h][0] == 'stmt']";
    let second = format!("world.lookup({heads}[1])");
    let first = format!("world.lookup({heads}[0])");
    ed.drag(ed.center(&second), ed.center(&first));
    ed.idle(4);
    let after = ed.eval::<String>("editor.source('game')");
    assert!(
        after.find("pos.y +=").unwrap() < after.find("pos.x +=").unwrap(),
        "{after}"
    );
    // The note that sat above the first statement went with it.
    assert!(
        after.contains("    # one tick is a sixtieth of a second\n    pos.x +="),
        "{after}"
    );
    assert_ne!(before, after);
}

#[test]
fn dragging_a_palette_block_onto_a_statement_inserts_it_there() {
    let ed = editor();
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Control'; view_dirty[0] = True");
    ed.idle(3);
    let pass = "handles[('palette', 'pass')]";
    let target = "world.lookup([h for h in dnd if dnd[h][0] == 'stmt'][1])";
    ed.drag(ed.center(pass), ed.center(target));
    ed.idle(4);
    let after = ed.eval::<String>("editor.source('game')");
    assert!(
        after.contains("    pos.x += vel.x * 0.1\n    pass\n    pos.y +="),
        "{after}"
    );
}

#[test]
fn dragging_a_stacks_grip_saves_its_position_in_the_source() {
    let ed = editor();
    tab(&ed, 2);
    let [x, y, w, h] = ed.rect("handles[('grip', 'move')]");
    let from = [x + w / 2.0, y + h / 2.0];
    ed.drag(from, [from[0] + 30.0, from[1] + 20.0]);
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(src.contains("@sage.position(40, 30)"), "{src}");
    // The comment above the stack is still above the decorators.
    assert!(
        src.contains("# Moves everything that has a velocity.\n@sage.position(40, 30)"),
        "{src}"
    );
}

#[test]
fn a_runtime_error_is_shown_on_the_block_that_raised_it() {
    let ed = editor();
    ed.run("editor.set_source('game', editor.source('game').replace('pos.y += vel.y * 0.1', 'pos.y += nope'))");
    ed.click("b_step");
    ed.idle(4);
    tab(&ed, 2);
    ed.idle(2);
    let t = texts(&ed);
    assert!(t.contains("WHY:") && t.contains("NOPE"), "{t}");
}

#[test]
fn lambdas_and_comprehensions_are_blocks_and_types_are_square() {
    let ed = editor();
    ed.run("editor.set_source('game', editor.source('game') + '\\n\\n@world.system(\"pick\", query=\"Position\")\\ndef pick(pos: Position):\\n    xs = [i * 2 for i in range(3) if i > 0]\\n    f = lambda a: a + 1\\n    n: int = 3\\n')");
    ed.idle(4);
    tab(&ed, 2);
    let t = texts(&ed);
    for want in ["LIST OF", "FOR", "LAMBDA", "DECLARE N"] {
        assert!(t.contains(want), "{want}: {t}");
    }
}

/// Scrolls the block canvas with the wheel.
fn scroll_canvas(ed: &Editor, notches: f32) {
    let c = ed.center("handles['canvas']");
    ed.input(&sage_platform::ui_window::UiTickInput {
        pointer: Some(c),
        wheel: notches,
        ..Default::default()
    });
    ed.idle(2);
}

fn with_source(ed: &Editor, extra: &str) {
    ed.run(&format!(
        "editor.set_source('game', editor.source('game') + {extra:?})"
    ));
    ed.idle(4);
}

#[test]
fn plus_buttons_grow_lists_dicts_and_calls_and_tuples_stay_fixed() {
    let ed = editor();
    ed.run("editor.set_source('game', B.Blocks(editor.source('game')).delete_function('move'))");
    ed.idle(3);
    with_source(
        &ed,
        "\n\n@world.system(\"grow\", query=\"Position\")\ndef grow(pos: Position):\n    c = print(1)\n    a = [1]\n    b = {'k': 1}\n    d = (1, 2)\n",
    );
    tab(&ed, 2);
    ed.click("handles[('grow', 'list', 'items')]");
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(src.contains("a = [1, 0]"), "{src}");
    scroll_canvas(&ed, 2.0);
    ed.click("handles[('grow', 'dict', 'items')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("b = {'k': 1, 'key': 0}")
    );
    ed.click("handles[('grow', 'call', 'args')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("c = print(1, 0)")
    );
    ed.click("handles[('grow', 'call', 'kwargs')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("c = print(1, 0, arg=0)")
    );
    assert!(
        ed.eval::<bool>("not any(k[:2] == ('grow', 'tuple') for k in handles)"),
        "tuples have no plus"
    );
}

#[test]
fn a_method_name_is_a_free_identifier_checked_on_commit() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"m\", query=\"Position\")\ndef m(pos: Position):\n    items = []\n    items.append(1)\n",
    );
    tab(&ed, 2);
    let name = "handles[('method', 'append')]";
    ed.click(name);
    ed.press(NAV_SELECT_ALL);
    ed.type_text("extend");
    ed.press(NAV_ACCEPT);
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("items.extend(1)")
    );
    let ok = ed.eval::<String>("editor.source('game')");
    ed.click("handles[('method', 'extend')]");
    ed.press(NAV_SELECT_ALL);
    ed.type_text("class");
    ed.press(NAV_ACCEPT);
    ed.idle(4);
    assert_eq!(
        ed.eval::<String>("editor.source('game')"),
        ok,
        "a keyword is not a method name"
    );
}

#[test]
fn a_slot_that_wants_a_test_refuses_a_plain_value_and_takes_a_test() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"t\", query=\"Position\")\ndef t(pos: Position):\n    if True:\n        pass\n",
    );
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Values'; view_dirty[0] = True");
    ed.idle(3);
    let before = ed.eval::<String>("editor.source('game')");
    let slot = "world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'bool'][0])";
    ed.drag(
        ed.center("handles[('palette', 'whole number')]"),
        ed.center(slot),
    );
    ed.idle(4);
    assert_eq!(
        ed.eval::<String>("editor.source('game')"),
        before,
        "a number is not a test"
    );
    assert!(texts(&ed).contains("WANTS A TEST"), "{}", texts(&ed));
    ed.drag(ed.center("handles[('palette', 'true')]"), ed.center(slot));
    ed.idle(4);
    // `true` is a test, and it is what was already there; try a comparison.
    ed.run("blk['cat'] = 'Operators'; view_dirty[0] = True");
    ed.idle(3);
    ed.drag(
        ed.center("handles[('palette', '[] < []')]"),
        ed.center(slot),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("if 0 < 0:")
    );
}

#[test]
fn blocks_glow_while_the_game_runs_and_pause_holds_it() {
    let ed = editor();
    tab(&ed, 2);
    ed.click("b_play");
    ed.idle(2);
    let lit = ed.eval::<usize>("len(glow['lit'])");
    assert!(lit >= 2, "the two statements of `move` ran: {lit}");
    ed.click("b_pause");
    ed.idle(3);
    let t = ed.project(|p| p.tick);
    ed.idle(20);
    assert_eq!(ed.project(|p| p.tick), t, "paused: no ticks");
    assert!(texts(&ed).contains("RESUME"));
    ed.click("b_step");
    ed.idle(2);
    assert_eq!(
        ed.project(|p| p.tick),
        t + 1,
        "step advances one tick while paused"
    );
    // The run did not change what is authored.
    ed.click("b_play");
    ed.idle(3);
    assert!(
        ed.project(|p| p.live_scene().write())
            .contains("Position x=20.0")
    );
}

const TIMED: &str = "\n\nDetect = world.phase('Detect')\nResolve = world.phase('Resolve', after=Detect)\nEnemy = world.prefab('Enemy')\nBoss = world.prefab('Boss', base=Enemy)\n\n@world.system('late', phase=Resolve, query='Position')\ndef late(pos: Position):\n    pass\n";

#[test]
fn the_timing_tab_edits_what_runs_after_what_and_refuses_cycles() {
    let ed = editor();
    // Declarations go before the systems that use them.
    with_source(&ed, TIMED);
    tab(&ed, 3);
    let t = texts(&ed);
    assert!(
        t.contains("DETECT") && t.contains("RESOLVE") && t.contains("SYSTEMS: LATE"),
        "{t}"
    );
    // Make Detect run after Resolve: a cycle, refused.
    let before = ed.eval::<String>("editor.source('game')");
    ed.run("editor.request('x') if False else None");
    ed.run("on_select[handles[('phase', 'Detect')].sid](1)");
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    assert!(texts(&ed).contains("DEPEND ON ITSELF"), "{}", texts(&ed));
    // Resolve after nothing: the dependency goes, and so does the arrow.
    ed.run("on_select[handles[('phase', 'Resolve')].sid](0)");
    ed.idle(4);
    let after = ed.eval::<String>("editor.source('game')");
    assert!(
        after.contains("Resolve = world.phase('Resolve')"),
        "{after}"
    );
}

#[test]
fn the_prefab_tab_lists_prefabs_with_bases_and_their_systems() {
    let ed = editor();
    with_source(&ed, TIMED);
    tab(&ed, 4);
    let t = texts(&ed);
    assert!(
        t.contains("ENEMY") && t.contains("BOSS") && t.contains("INSTANCES"),
        "{t}"
    );
    ed.run("on_select[handles[('prefab', 'Enemy')].sid](0)");
    ed.run("on_select[handles[('prefab', 'Boss')].sid](0)");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("Boss = world.prefab('Boss')")
    );
}

#[test]
fn a_system_hats_dropdown_sets_its_phase() {
    let ed = editor();
    with_source(&ed, TIMED);
    tab(&ed, 2);
    ed.run("on_select[handles[('system_phase', 'move')].sid](1)");
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(
        src.contains("world.system('move', query='Position, Velocity', phase=Detect)")
            || src.contains("phase=Detect"),
        "{src}"
    );
}

#[test]
fn type_blocks_go_in_type_slots_only_and_any_is_marked() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"ty\", query=\"Position\")\ndef ty(pos: Position):\n    n: int = 3\n    m = 4\n",
    );
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Types'; view_dirty[0] = True");
    ed.idle(3);
    let slot = "world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'type'][0])";
    ed.drag(
        ed.center("handles[('palette', 'any (checking off)')]"),
        ed.center(slot),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("n: Any = 3")
    );
    assert!(texts(&ed).contains("ANY ?") && texts(&ed).contains("CHECKING OFF"));
    // A type is refused where a value goes.
    let before = ed.eval::<String>("editor.source('game')");
    let value = "world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] is None][-1])";
    ed.drag(ed.center("handles[('palette', 'int')]"), ed.center(value));
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    // And a value is refused in a type slot.
    ed.run("blk['cat'] = 'Values'; view_dirty[0] = True");
    ed.idle(3);
    ed.drag(ed.center("handles[('palette', 'text')]"), ed.center(slot));
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
}

#[test]
fn a_stack_whose_query_lost_a_component_greys_its_blocks_and_offers_a_fix() {
    let ed = editor();
    ed.run("editor.set_source('game', editor.source('game').replace('query=\"Position, Velocity\"', 'query=\"Position\"'))");
    ed.idle(4);
    tab(&ed, 2);
    assert!(
        texts(&ed).contains("VELOCITY NOT IN QUERY"),
        "{}",
        texts(&ed)
    );
    ed.click("handles[('fix_query', 'Velocity')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("Position, Velocity'")
    );
    assert!(!texts(&ed).contains("NOT IN QUERY"));
}

#[test]
fn the_hat_dropdown_uses_relative_phase_names() {
    let ed = editor();
    with_source(&ed, TIMED);
    tab(&ed, 2);
    let labels = ed.eval::<String>(
        "str([e for e in B.relative_phases(B.declarations(editor.source('game')))])",
    );
    assert!(
        labels.contains("before Resolve") && labels.contains("at Detect"),
        "{labels}"
    );
    // `before Resolve` is the phase Resolve needs: Detect.
    ed.run("on_select[handles[('system_phase', 'late')].sid](3)");
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(src.contains("phase=Detect"), "{src}");
}

#[test]
fn a_stray_stack_runs_whole_by_button_or_double_click_and_up_to_a_block() {
    let ed = editor();
    with_source(
        &ed,
        "\n\ndef spawn_two():\n    world.entity('a', 10)\n    world.entity('b', 11)\n",
    );
    tab(&ed, 2);
    assert!(ed.eval::<bool>("('run', 'spawn_two') in handles"));
    let n = ed.eval::<usize>("len(editor.outliner())");
    // Run up to the first block only.
    ed.click("handles[('stmt', 3, 'run_to')]");
    ed.idle(4);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n + 1);
    // Two quick clicks on the title run the whole stack (b is new, a again fails: names clash).
    ed.run("editor.undo()");
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n);
    ed.click("handles[('run', 'spawn_two')]");
    ed.click("handles[('run', 'spawn_two')]");
    ed.idle(4);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n + 2);
    // What it did is one edit, and it undoes.
    ed.run("editor.undo()");
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("len(editor.outliner())"), n);
}

const MODS: &str = "\n\nDetect = world.phase('Detect')\nResolve = world.phase('Resolve', after=Detect)\nPost = world.phase('Post', after=Resolve)\ncombat = world.module('combat')\n\n@combat.system('hit', phase=Detect, query='Position')\ndef hit(pos: Position):\n    pass\n\n@world.system('plain', query='Position')\ndef plain(pos: Position):\n    pass\n";

#[test]
fn dragging_an_out_handle_onto_a_phase_makes_it_run_after_and_dims_cycles() {
    let ed = editor();
    with_source(&ed, MODS);
    tab(&ed, 3);
    // Post runs after Resolve; making Detect run after Post would be a cycle.
    let before = ed.eval::<String>("editor.source('game')");
    ed.drag(
        ed.center("handles[('phase_out', 'Post')]"),
        ed.center("handles[('phase_card', 'Detect')]"),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    assert!(texts(&ed).contains("DEPEND ON ITSELF"));
    // Post after Detect directly is fine: Post stops waiting for Resolve.
    ed.drag(
        ed.center("handles[('phase_out', 'Detect')]"),
        ed.center("handles[('phase_card', 'Post')]"),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("Post = world.phase('Post', after=Detect)")
    );
}

#[test]
fn hovering_a_phase_labels_its_dependents_by_hops_and_pulses() {
    let ed = editor();
    with_source(&ed, MODS);
    tab(&ed, 3);
    let c = ed.center("handles[('phase_card', 'Detect')]");
    ed.input(&sage_platform::ui_window::UiTickInput {
        pointer: Some(c),
        ..Default::default()
    });
    for _ in 0..40 {
        ed.input(&sage_platform::ui_window::UiTickInput {
            pointer: Some(c),
            ..Default::default()
        });
    }
    let t = texts(&ed);
    assert!(
        t.contains("HOVER") && t.contains("+1") && t.contains("+2"),
        "{t}"
    );
    let lit = ed.eval::<usize>("sum(1 for v in timing_state['shown'].values() if v[1])");
    assert!(lit <= 1, "one hop pulses at a time: {lit}");
    // It rests after a few cycles, leaving the static badges.
    for _ in 0..120 {
        ed.input(&sage_platform::ui_window::UiTickInput {
            pointer: Some(c),
            ..Default::default()
        });
    }
    assert_eq!(
        ed.eval::<usize>("sum(1 for v in timing_state['shown'].values() if v[1])"),
        0
    );
    assert!(texts(&ed).contains("+2"));
}

#[test]
fn the_crosstab_moves_systems_between_cells_and_says_why_a_module_is_off() {
    let ed = editor();
    with_source(&ed, MODS);
    tab(&ed, 3);
    // `plain` (script, default phase) moves to the combat row, Resolve column.
    ed.drag(
        ed.center("handles[('sys', 'plain')]"),
        ed.center("handles[('cell', 'combat', 'Resolve')]"),
    );
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(
        src.contains("@combat.system('plain', query='Position', phase=Resolve)"),
        "{src}"
    );
    // Disabling the module turns its systems off, and each says why.
    ed.click("handles[('module_toggle', 'combat')]");
    ed.idle(5);
    assert!(
        texts(&ed).contains("OFF: MODULE COMBAT IS DISABLED"),
        "{}",
        texts(&ed)
    );
    ed.click("handles[('module_toggle', 'combat')]");
    ed.idle(5);
    assert!(!texts(&ed).contains("IS DISABLED"));
}

const FLAT: &str = "\n\n@world.system(\"flat\", query=\"Position\")\ndef flat(pos: Position):\n    a = 1\n    return a\n";

#[test]
fn return_break_and_continue_have_flat_bottoms_and_nothing_goes_after_them() {
    let ed = editor();
    with_source(&ed, FLAT);
    tab(&ed, 2);
    // `move` ends in a bar for more blocks; `flat` ends in return, so it has none.
    let bars = ed.eval::<usize>("sum(1 for k in handles if k[0] == 'slot')");
    assert_eq!(
        bars, 1,
        "only the stack that does not end in return has a bar"
    );
    // Select the return block and try to add a block after it: refused.
    ed.run("blk['cat'] = 'Control'; view_dirty[0] = True");
    ed.idle(3);
    let before = ed.eval::<String>("editor.source('game')");
    ed.click("handles[('pick', 4)]"); // statements: move 1 and 2, then a = 1 (3), return (4)
    ed.idle(3);
    ed.click("handles[('palette', 'pass')]");
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    assert!(
        texts(&ed).contains("NOTHING CAN GO AFTER RETURN"),
        "{}",
        texts(&ed)
    );
    // Dragging a block onto the return's head inserts it *before* the return.
    ed.drag(
        ed.center("handles[('palette', 'pass')]"),
        ed.center("world.lookup([h for h in dnd if dnd[h][0] == 'stmt'][3])"),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("    a = 1\n    pass\n    return a\n")
    );
    // And a `return` cannot be dropped anywhere but at the end of a body.
    let b2 = ed.eval::<String>("editor.source('game')");
    ed.drag(
        ed.center("handles[('palette', 'return')]"),
        ed.center("world.lookup([h for h in dnd if dnd[h][0] == 'stmt'][0])"),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), b2);
}

#[test]
fn enter_steps_through_blocks_depth_first_and_lands_on_empty_branches() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"br\", query=\"Position\")\ndef br(pos: Position):\n    if True:\n        pass\n    x = 1\n",
    );
    tab(&ed, 2);
    // The empty `if` body is lit: a bar says "+ HERE".
    assert!(texts(&ed).contains("+ HERE"), "{}", texts(&ed));
    // `move` has statements 1 and 2; `if` is 3, its `pass` 4, then the empty
    // branch's bar, then `x = 1`.
    ed.click("handles[('pick', 3)]");
    ed.idle(3);
    assert_eq!(ed.eval::<String>("blk['sel'][0]"), "s");
    ed.press(NAV_ACCEPT);
    ed.idle(3);
    // Reading order: move's two blocks and its bar, then if (3), pass (4),
    // the branch's bar (5), x = 1.
    let n = ed.eval::<usize>("view_order_index()");
    assert_eq!(n, 4, "Enter went to the next block");
    // Focus follows the selection.
    let focus = ed.eval::<u64>("world.get(Interaction).focus");
    let want = ed.eval::<u64>("handles[('pick', 4)].sid");
    assert_eq!(focus, want);
    // Shift+Enter goes back.
    ed.input(&sage_platform::ui_window::UiTickInput {
        nav: NAV_ACCEPT | NAV_SHIFT,
        ..Default::default()
    });
    ed.idle(3);
    assert_eq!(ed.eval::<usize>("view_order_index()"), 3);
    // Enter twice lands on the bottom bar of the branch, and the palette
    // fills it.
    ed.press(NAV_ACCEPT);
    ed.idle(2);
    ed.press(NAV_ACCEPT);
    ed.idle(3);
    assert_eq!(
        ed.eval::<String>("blk['sel'][0]"),
        "b",
        "the empty branch's bar is selected"
    );
    ed.run("blk['cat'] = 'Control'; view_dirty[0] = True");
    ed.idle(3);
    ed.click("handles[('palette', 'break')]");
    ed.idle(4);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(
        src.contains("    if True:\n        pass\n        break\n    x = 1\n"),
        "{src}"
    );
}

#[test]
fn ctrl_b_and_ctrl_e_open_the_palettes_and_enter_inserts_the_first_match() {
    let ed = editor();
    ed.chord(b'B' as u32, CHORD_CTRL);
    assert_eq!(ed.eval::<String>("blk['mode']"), "block");
    assert_eq!(
        ed.eval::<i64>("ui.selected_tab(tabs_root)"),
        2,
        "it opens the block tab"
    );
    assert_eq!(
        ed.eval::<u64>("world.get(Interaction).focus"),
        ed.eval::<u64>("pal_filter.sid")
    );
    ed.type_text("whi");
    ed.idle(2);
    assert_eq!(ed.eval::<usize>("len(blk['entries'])"), 1);
    ed.click("handles[('pick', 1)]");
    ed.idle(2);
    ed.run("pal_filter.set(Label(0))");
    ed.chord(b'B' as u32, CHORD_CTRL);
    ed.type_text("whi");
    ed.press(NAV_ACCEPT);
    ed.idle(3);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(src.contains("    while True:\n        pass\n"), "{src}");
    // Ctrl+E lists values only.
    ed.chord(b'E' as u32, CHORD_CTRL);
    assert_eq!(ed.eval::<String>("blk['mode']"), "expr");
    assert!(ed.eval::<bool>("all(e['shape'] != 'command' for e in blk['entries'])"));
    // Esc closes it.
    ed.press(NAV_CANCEL);
    ed.idle(2);
    assert!(ed.eval::<bool>("blk['mode'] is None"));
}

#[test]
fn block_shortcuts_move_delete_note_and_add_else_on_the_selection() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"k\", query=\"Position\")\ndef k(pos: Position):\n    a = 1\n    b = 2\n    if a:\n        pass\n",
    );
    tab(&ed, 2);
    ed.click("handles[('pick', 3)]"); // a = 1 (after move's two)
    ed.idle(3);
    ed.chord(sage_core::host::CHORD_DOWN, CHORD_ALT);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(src.contains("    b = 2\n    a = 1\n    if a:"), "{src}");
    ed.click("handles[('pick', 4)]");
    ed.idle(3);
    ed.chord(sage_core::host::CHORD_UP, CHORD_ALT);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("    a = 1\n    b = 2\n    if a:")
    );
    ed.chord(b'N' as u32, CHORD_ALT);
    ed.idle(2);
    ed.click("handles[('pick', 5)]"); // the if
    ed.idle(3);
    ed.chord(b'L' as u32, CHORD_ALT);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("    if a:\n        pass\n    else:\n        pass\n")
    );
    ed.click("handles[('pick', 4)]");
    ed.idle(3);
    ed.chord(sage_core::host::CHORD_DELETE, 0);
    let src = ed.eval::<String>("editor.source('game')");
    assert!(!src.contains("b = 2"), "{src}");
    // Esc deselects.
    ed.press(NAV_CANCEL);
    ed.idle(2);
    assert!(ed.eval::<bool>("blk['sel'] is None"));
}

#[test]
fn ctrl_digits_switch_tabs_and_f6_cycles_panes() {
    let ed = editor();
    ed.chord(b'2' as u32, CHORD_CTRL);
    assert_eq!(ed.eval::<i64>("ui.selected_tab(tabs_root)"), 1);
    ed.chord(b'3' as u32, CHORD_CTRL);
    assert_eq!(ed.eval::<i64>("ui.selected_tab(tabs_root)"), 2);
    ed.chord(b'1' as u32, CHORD_CTRL);
    assert_eq!(ed.eval::<i64>("ui.selected_tab(tabs_root)"), 0);
    let f6 = sage_core::host::CHORD_F1 + 5;
    ed.chord(f6, 0);
    let a = ed.eval::<u64>("world.get(Interaction).focus");
    ed.chord(f6, 0);
    let b = ed.eval::<u64>("world.get(Interaction).focus");
    assert_ne!(a, b, "F6 moves to another pane");
    ed.chord(f6, CHORD_SHIFT);
    assert_eq!(
        ed.eval::<u64>("world.get(Interaction).focus"),
        a,
        "Shift+F6 goes back"
    );
}

#[test]
fn blocks_are_named_for_a_screen_reader() {
    let ed = editor();
    tab(&ed, 2);
    let outline = ed.access_outline();
    assert!(
        outline.contains("BLOCK 1: POS.X += VEL.X * 0.1"),
        "{outline}"
    );
    assert!(outline.contains("END OF BODY BODY"), "{outline}");
}

/// A point on the left padding of the value box `expr` evaluates to: a drop
/// there reaches the box itself, not the field inside it.
fn edge_of(ed: &Editor, expr: &str) -> [f32; 2] {
    let [x, y, _, h] = ed.rect(expr);
    [x + 1.0, y + h / 2.0]
}

fn slot_where(want: &str, value: &str) -> String {
    format!(
        "world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == {want:?} and cur['view'].b.find(dnd[h][1])[0].get('v') == {value}][0])"
    )
}

#[test]
fn typed_sockets_refuse_the_wrong_kind_of_value_and_accept_unknowns() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"ts\", query=\"Position\")\ndef ts(pos: Position):\n    a = 2 * 3\n    s: str = 'x'\n    n: int = 4\n",
    );
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Values'; view_dirty[0] = True");
    ed.idle(3);
    let before = ed.eval::<String>("editor.source('game')");
    // A number slot (an operand of `*`) refuses text.
    let num = slot_where("num", "2");
    ed.drag(
        ed.center("handles[('palette', 'text')]"),
        edge_of(&ed, &num),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    assert!(texts(&ed).contains("WANTS A NUMBER"), "{}", texts(&ed));
    // ...but takes a number.
    ed.drag(
        ed.center("handles[('palette', 'decimal number')]"),
        edge_of(&ed, &num),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("a = 0.0 * 3")
    );
    // A text slot (`s: str = ...`) refuses a number.
    let b2 = ed.eval::<String>("editor.source('game')");
    let text = slot_where("str", "'x'");
    ed.drag(
        ed.center("handles[('palette', 'whole number')]"),
        edge_of(&ed, &text),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), b2);
    assert!(texts(&ed).contains("WANTS TEXT"), "{}", texts(&ed));
    // An unknown value (a call) fits anywhere.
    ed.run("blk['cat'] = 'Calls'; view_dirty[0] = True");
    ed.idle(3);
    let num = slot_where("num", "0.0");
    ed.drag(
        ed.center("handles[('palette', 'call [] ( )')]"),
        edge_of(&ed, &num),
    );
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("a = f() * 3")
    );
}

#[test]
fn delete_removes_the_selected_block_but_not_while_typing_in_a_field() {
    let ed = editor();
    with_source(
        &ed,
        "\n\n@world.system(\"dd\", query=\"Position\")\ndef dd(pos: Position):\n    a = 1\n    b = 2\n",
    );
    tab(&ed, 2);
    ed.click("handles[('pick', 3)]");
    ed.idle(3);
    // Focus in the palette filter: Delete is text editing, not block deletion.
    ed.run("set_focus(pal_filter)");
    ed.idle(1);
    ed.chord(sage_core::host::CHORD_DELETE, 0);
    assert!(ed.eval::<String>("editor.source('game')").contains("a = 1"));
    // With focus on the block, it deletes.
    ed.run("set_focus(handles[('pick', 3)])");
    ed.idle(1);
    ed.chord(sage_core::host::CHORD_DELETE, 0);
    assert!(!ed.eval::<String>("editor.source('game')").contains("a = 1"));
}

#[test]
fn the_hover_delay_and_pulse_count_are_settings() {
    let ed = editor();
    with_source(&ed, MODS);
    tab(&ed, 3);
    ed.click("handles[('setting', 'hover_delay')]");
    ed.press(NAV_SELECT_ALL);
    ed.type_text("40");
    ed.press(NAV_ACCEPT);
    ed.idle(3);
    assert_eq!(ed.eval::<i64>("settings['hover_delay']"), 40);
    let c = ed.center("handles[('phase_card', 'Detect')]");
    let hover = |n: usize| {
        for _ in 0..n {
            ed.input(&sage_platform::ui_window::UiTickInput {
                pointer: Some(c),
                ..Default::default()
            });
        }
    };
    hover(30);
    assert!(!texts(&ed).contains("+1"), "still inside the longer delay");
    hover(20);
    assert!(texts(&ed).contains("+1"), "{}", texts(&ed));
}

const GRIDS: &str = "\n\n@world.system(\"gr\", query=\"Position\")\ndef gr(pos: Position):\n    tiles: list[list[int]] = [[1, 0], [0, 1]]\n    names: list[str] = ['a', 'b']\n    free = [1, 2]\n";

#[test]
fn a_list_of_lists_is_an_editable_grid_that_grows_by_row_and_column() {
    let ed = editor();
    with_source(&ed, GRIDS);
    tab(&ed, 2);
    assert!(texts(&ed).contains("GRID 2X2"), "{}", texts(&ed));
    ed.click("handles[('grid', '+ROW')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("tiles: list[list[int]] = [[1, 0], [0, 1], [0, 0]]")
    );
    assert!(texts(&ed).contains("GRID 3X2"));
    ed.click("handles[('grid', '+COL')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("[[1, 0, 0], [0, 1, 0], [0, 0, 0]]")
    );
    ed.click("handles[('grid', '-ROW')]");
    ed.click("handles[('grid', '-COL')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("[[1, 0], [0, 1]]")
    );
}

#[test]
fn typed_lists_take_only_their_element_type_and_grow_with_a_matching_default() {
    let ed = editor();
    ed.run("editor.set_source('game', B.Blocks(editor.source('game')).delete_function('move'))");
    ed.idle(3);
    with_source(
        &ed,
        GRIDS
            .replace("    tiles: list[list[int]] = [[1, 0], [0, 1]]\n", "")
            .replace("    free = [1, 2]\n", "")
            .as_str(),
    );
    tab(&ed, 2);
    ed.run("blk['cat'] = 'Values'; view_dirty[0] = True");
    ed.idle(3);
    // `names: list[str]`: its elements want text; a number is refused.
    let before = ed.eval::<String>("editor.source('game')");
    let slot = slot_where("str", "'a'");
    ed.drag(
        ed.center("handles[('palette', 'whole number')]"),
        edge_of(&ed, &slot),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), before);
    assert!(texts(&ed).contains("WANTS TEXT"), "{}", texts(&ed));
    // Its plus adds empty text, not a number.
    ed.click("handles[('grow', 'list', 'items')]");
    ed.idle(4);
    assert!(
        ed.eval::<String>("editor.source('game')")
            .contains("names: list[str] = ['a', 'b', '']"),
        "{}",
        ed.eval::<String>("editor.source('game')")
    );
    // A list of numbers is refused where a list of text is wanted, and a
    // list of text is not refused for `names` as a whole.
    let names =
        "world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'list<str>'][0])";
    let b2 = ed.eval::<String>("editor.source('game')");
    ed.drag(
        ed.center("handles[('palette', 'list of numbers')]"),
        edge_of(&ed, names),
    );
    ed.idle(4);
    assert_eq!(ed.eval::<String>("editor.source('game')"), b2);
    assert!(
        texts(&ed).contains("WANTS A LIST OF TEXT"),
        "{}",
        texts(&ed)
    );
}

#[test]
fn suggestions_narrow_to_what_fits_the_selected_slot_unless_the_project_opts_out() {
    let ed = editor();
    ed.run("editor.set_source('game', B.Blocks(editor.source('game')).delete_function('move'))");
    ed.idle(3);
    with_source(
        &ed,
        "\n\n@world.system(\"sg\", query=\"Position\")\ndef sg(pos: Position):\n    if True:\n        pass\n    n: int = 3\n",
    );
    tab(&ed, 2);
    ed.run("blk['mode'] = 'expr'; view_dirty[0] = True");
    ed.idle(3);
    let all = ed.eval::<usize>("len(blk['entries'])");
    assert!(all > 10);
    // Select the `if` test: a slot that wants a test. Only tests (and
    // unknown values) are suggested.
    ed.click("world.lookup([h for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'bool'][0]).children[0]");
    ed.idle(3);
    ed.run("blk['sel'] = ('e', 'sg', [dnd[h][1] for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'bool'][0]); view_dirty[0] = True");
    ed.idle(3);
    let narrowed = ed.eval::<usize>("len(blk['entries'])");
    assert!(narrowed < all && narrowed > 0, "{narrowed} of {all}");
    assert!(ed.eval::<bool>("all(e['shape'] in ('predicate', 'reporter') and (e['shape'] == 'predicate' or B.type_of(e['block']) is None) for e in blk['entries'])"));
    assert!(
        texts(&ed).contains("SHOWING WHAT FITS THE SELECTION: A TEST"),
        "{}",
        texts(&ed)
    );
    // A number slot (`n: int = 3`) suggests numbers.
    ed.run("blk['sel'] = ('e', 'sg', [dnd[h][1] for h in dnd if dnd[h][0] == 'expr' and dnd[h][2] == 'num'][0]); view_dirty[0] = True");
    ed.idle(3);
    assert!(ed.eval::<bool>("any(e['label'] == 'whole number' for e in blk['entries']) and not any(e['label'] == 'text' for e in blk['entries'])"));
    // The project can turn it off: the whole list comes back, and the
    // setting is saved with the project.
    ed.click("suggest_toggle");
    ed.idle(3);
    assert_eq!(
        ed.eval::<String>("editor.setting('suggest_by_type', 'on')"),
        "off"
    );
    assert_eq!(ed.eval::<usize>("len(blk['entries'])"), all);
    assert!(!texts(&ed).contains("SHOWING WHAT FITS"));
    assert!(
        ed.project(|p| p.settings_text())
            .contains("suggest_by_type off")
    );
}
