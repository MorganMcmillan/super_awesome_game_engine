//! A settings menu built from the UI module's widgets (`scripts/menu.py`),
//! driven through the same input a window would give it and rendered with
//! the CPU reference rasterizer. The smallest UI application: a [`UiHost`]
//! and one script.

use sage_app::{UiHost, UiHosted};

pub const SCRIPT: &str = include_str!("../scripts/menu.py");

/// The UI size in UI units; a window shows it at a whole-number scale.
pub const SCREEN: [f32; 2] = [160.0, 144.0];

pub struct Demo {
    ui: UiHost,
}

impl std::ops::Deref for Demo {
    type Target = UiHost;
    fn deref(&self) -> &UiHost {
        &self.ui
    }
}

impl UiHosted for Demo {
    fn ui(&self) -> &UiHost {
        &self.ui
    }

    fn clear_color(&self) -> [u8; 4] {
        [12, 14, 18, 255]
    }
}

impl Demo {
    pub fn new() -> Result<Demo, String> {
        let mut ui = UiHost::new("menu", SCREEN)?;
        ui.load("menu", SCRIPT)?;
        let demo = Demo { ui };
        demo.step();
        Ok(demo)
    }

    /// The text of the status line.
    pub fn status(&self) -> String {
        self.eval("world.string(status.get(Label).text)")
    }

    /// Plays a short session: moves the volume, turns the music on, types a
    /// name and hovers over Play.
    pub fn play_session(&self) {
        let [x, y, w, h] = self.rect("volume");
        self.click_at([x + w * 0.7, y + h / 2.0]);
        self.click("music");
        self.click("name");
        self.type_text("Ada");
        self.hover(self.center("play"));
    }
}
