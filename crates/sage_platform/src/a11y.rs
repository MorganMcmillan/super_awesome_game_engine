//! The bridge to the platform's accessibility API.
//!
//! The UI makes an `accesskit::TreeUpdate` of what is on screen each tick and
//! takes the actions assistive technology asks for (focus, click). On
//! Windows this module connects them to the window through UI Automation
//! (`accesskit_windows`). On other platforms [`Accessibility::attach`] gives
//! `None` and the window runs without it: an adapter for them is not built.

use accesskit::{ActionRequest, TreeUpdate};

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::mpsc::{Receiver, Sender, channel};

    use accesskit::{
        ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, Role, TreeId, TreeInfo,
        TreeUpdate,
    };
    use accesskit_windows::{HWND, SubclassingAdapter};

    /// What the platform sees before the first real update: an empty window.
    struct Initial;

    impl ActivationHandler for Initial {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            let root = NodeId(u64::MAX);
            let mut info = TreeInfo::new(root);
            info.toolkit_name = Some("sage_ui".into());
            Some(TreeUpdate {
                nodes: vec![(root, Node::new(Role::Window))],
                tree: Some(info),
                tree_id: TreeId::ROOT,
                focus: root,
            })
        }
    }

    /// Actions arrive on a thread of the platform's choosing: hand them to
    /// the window's own thread over a channel.
    struct Actions(Sender<ActionRequest>);

    impl ActionHandler for Actions {
        fn do_action(&mut self, request: ActionRequest) {
            let _ = self.0.send(request);
        }
    }

    pub struct Adapter {
        inner: SubclassingAdapter,
        rx: Receiver<ActionRequest>,
    }

    impl Adapter {
        /// Subclasses the window `hwnd`. This must happen before the window is
        /// first shown.
        pub fn new(hwnd: *mut c_void) -> Adapter {
            let (tx, rx) = channel();
            let inner = SubclassingAdapter::new(HWND(hwnd), Initial, Actions(tx));
            Adapter { inner, rx }
        }

        pub fn update(&mut self, make: impl FnOnce() -> TreeUpdate) {
            if let Some(events) = self.inner.update_if_active(make) {
                events.raise();
            }
        }

        pub fn actions(&mut self) -> Vec<ActionRequest> {
            self.rx.try_iter().collect()
        }
    }
}

/// A tree with only the window: what an app that tells nothing shows.
pub fn empty_update() -> TreeUpdate {
    let root = accesskit::NodeId(u64::MAX);
    TreeUpdate {
        nodes: vec![(root, accesskit::Node::new(accesskit::Role::Window))],
        tree: Some(accesskit::TreeInfo::new(root)),
        tree_id: accesskit::TreeId::ROOT,
        focus: root,
    }
}

/// The accessibility connection of one window.
pub struct Accessibility {
    #[cfg(windows)]
    adapter: win::Adapter,
}

impl Accessibility {
    /// Connects `window` to the platform's accessibility API, if this
    /// platform has an adapter. Call it before the window is shown.
    pub fn attach(window: &sdl3::video::Window) -> Option<Accessibility> {
        #[cfg(windows)]
        {
            use sdl3::sys::properties::SDL_GetPointerProperty;
            use sdl3::sys::video::{SDL_GetWindowProperties, SDL_PROP_WINDOW_WIN32_HWND_POINTER};
            // SAFETY: the window is alive; the property is a plain pointer.
            let hwnd = unsafe {
                let props = SDL_GetWindowProperties(window.raw());
                SDL_GetPointerProperty(
                    props,
                    SDL_PROP_WINDOW_WIN32_HWND_POINTER,
                    std::ptr::null_mut(),
                )
            };
            if hwnd.is_null() {
                return None;
            }
            Some(Accessibility {
                adapter: win::Adapter::new(hwnd),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            None
        }
    }

    /// Gives the platform this tick's tree, if a screen reader (or other
    /// assistive tool) is listening; `make` is only called then.
    pub fn update(&mut self, make: impl FnOnce() -> TreeUpdate) {
        #[cfg(windows)]
        self.adapter.update(make);
        #[cfg(not(windows))]
        let _ = make;
    }

    /// The actions assistive technology has asked for since the last call.
    pub fn actions(&mut self) -> Vec<ActionRequest> {
        #[cfg(windows)]
        return self.adapter.actions();
        #[cfg(not(windows))]
        vec![]
    }
}
