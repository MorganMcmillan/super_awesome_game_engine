//! How long the editor takes to do what a person makes it do: an idle tick, a
//! rebuild of the views on each tab, drawing, and a tick while playing.
//! `cargo run --release -p editor --example bench`

use editor::*;
use sage_app::UiHosted;
use std::time::Instant;
fn main() {
    let ed = Editor::new(sample_project().unwrap(), None).unwrap();
    ed.idle(3);
    let t = |label: &str, n: u32, f: &dyn Fn()| {
        let s = Instant::now();
        for _ in 0..n {
            f();
        }
        println!(
            "{label}: {:.2} ms each",
            s.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
    };
    t("idle tick (scene tab)", 60, &|| ed.idle(1));
    t("forced rebuild (scene tab)", 20, &|| {
        ed.run("view_dirty[0] = True");
        ed.idle(1);
    });
    ed.click("tab_buttons[2]");
    ed.idle(3);
    t("idle tick (blocks tab)", 60, &|| ed.idle(1));
    t("forced rebuild (blocks tab)", 20, &|| {
        ed.run("view_dirty[0] = True");
        ed.idle(1);
    });
    ed.click("tab_buttons[3]");
    ed.idle(3);
    t("forced rebuild (timing tab)", 20, &|| {
        ed.run("view_dirty[0] = True");
        ed.idle(1);
    });
    ed.click("tab_buttons[0]");
    ed.idle(3);
    t("ui_frame (compose draws)", 20, &|| {
        let _ = ed.frame();
    });
    t("live reload (a block edit does this)", 10, &|| {
        ed.run("editor.set_source('game', editor.source('game') + '\\n')");
        ed.idle(1);
    });
    ed.click("b_play");
    t("tick while playing", 60, &|| ed.idle(1));
}
