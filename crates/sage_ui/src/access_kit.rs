//! The accessibility tree as an `accesskit` update.
//!
//! [`crate::access`] derives plain data from the UI; this maps it onto
//! `accesskit`'s model, the form a platform adapter (UI Automation on
//! Windows, AT-SPI on Linux, NSAccessibility on macOS) consumes. It is a pure
//! function: it opens no connection and touches no window. Connecting an
//! adapter to the SDL window is the host's job, and is not done here.

use accesskit::{Action, Node, NodeId, Rect, Role as K, Toggled, TreeId, TreeInfo, TreeUpdate};

use crate::access::{AccessNode, Role};

/// The id of the window node every update is rooted at. UI nodes use their
/// StableId, which is never this.
pub const ROOT: NodeId = NodeId(u64::MAX);

fn role(r: Role) -> K {
    match r {
        Role::Generic => K::GenericContainer,
        Role::Button => K::Button,
        Role::Checkbox => K::CheckBox,
        Role::Radio => K::RadioButton,
        Role::Switch => K::Switch,
        Role::Slider => K::Slider,
        Role::TextBox => K::TextInput,
        Role::Link => K::Link,
        Role::Heading => K::Heading,
        Role::List => K::List,
        Role::ListItem => K::ListItem,
        Role::Table => K::Table,
        Role::Row => K::Row,
        Role::Cell => K::Cell,
        Role::ColumnHeader => K::ColumnHeader,
        Role::Tab => K::Tab,
        Role::TabList => K::TabList,
        Role::TabPanel => K::TabPanel,
        Role::Tree => K::Tree,
        Role::TreeItem => K::TreeItem,
        Role::Menu => K::Menu,
        Role::MenuBar => K::MenuBar,
        Role::MenuItem => K::MenuItem,
        Role::Dialog => K::Dialog,
        Role::Tooltip => K::Tooltip,
        Role::ProgressBar => K::ProgressIndicator,
        Role::Separator => K::Splitter,
        Role::Navigation => K::Navigation,
        Role::Banner => K::Banner,
        Role::ContentInfo => K::ContentInfo,
        Role::Main => K::Main,
        Role::Region => K::Region,
        Role::Form => K::Form,
        Role::Group => K::Group,
        Role::Image => K::Image,
        Role::Text => K::Label,
        Role::ComboBox => K::ComboBox,
        Role::ListBox => K::ListBox,
        Role::Option => K::ListBoxOption,
        Role::RadioGroup => K::RadioGroup,
        Role::Label => K::Label,
        Role::Application => K::Application,
    }
}

fn convert(n: &AccessNode, out: &mut Vec<(NodeId, Node)>) -> NodeId {
    let id = NodeId(n.id);
    let mut node = Node::new(role(n.role));
    if !n.name.is_empty() {
        node.set_label(n.name.clone());
    }
    if !n.description.is_empty() {
        node.set_description(n.description.clone());
    }
    if let Some(v) = &n.value {
        match n.role {
            Role::Slider | Role::ProgressBar => {
                if let Ok(x) = v.parse::<f64>() {
                    node.set_numeric_value(x);
                }
            }
            _ => node.set_value(v.clone()),
        }
    }
    if let Some(on) = n.on {
        match n.role {
            // A tab or a row of a tree is selected rather than checked.
            Role::Tab | Role::TreeItem | Role::Option => node.set_selected(on),
            _ => node.set_toggled(if on { Toggled::True } else { Toggled::False }),
        }
    }
    if n.level > 0 {
        node.set_level(n.level as usize);
    }
    if !n.enabled {
        node.set_disabled();
    }
    node.set_bounds(Rect {
        x0: f64::from(n.bounds[0]),
        y0: f64::from(n.bounds[1]),
        x1: f64::from(n.bounds[0] + n.bounds[2]),
        y1: f64::from(n.bounds[1] + n.bounds[3]),
    });
    if matches!(
        n.role,
        Role::Button
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Link
            | Role::Tab
            | Role::MenuItem
            | Role::Option
            | Role::TreeItem
            | Role::TextBox
            | Role::Slider
            | Role::ComboBox
    ) && n.enabled
    {
        node.add_action(Action::Focus);
        if !matches!(n.role, Role::TextBox | Role::Slider) {
            node.add_action(Action::Click);
        }
    }
    let children: Vec<NodeId> = n.children.iter().map(|c| convert(c, out)).collect();
    node.set_children(children);
    out.push((id, node));
    id
}

/// A full tree update: a window node over the UI's tree, the node with
/// `focus` focused (the window when none is).
pub fn tree_update(nodes: &[AccessNode], focus: u64, size: [f32; 2]) -> TreeUpdate {
    let mut out = vec![];
    let top: Vec<NodeId> = nodes.iter().map(|n| convert(n, &mut out)).collect();
    let mut window = Node::new(K::Window);
    window.set_bounds(Rect {
        x0: 0.0,
        y0: 0.0,
        x1: f64::from(size[0]),
        y1: f64::from(size[1]),
    });
    window.set_children(top);
    out.push((ROOT, window));
    let focus = if out.iter().any(|(id, _)| id.0 == focus) {
        NodeId(focus)
    } else {
        ROOT
    };
    let mut info = TreeInfo::new(ROOT);
    info.toolkit_name = Some(crate::CODENAME_UI.to_string());
    TreeUpdate {
        nodes: out,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u64, role: Role, name: &str) -> AccessNode {
        AccessNode {
            id,
            role,
            name: name.into(),
            description: String::new(),
            value: None,
            on: None,
            level: 0,
            enabled: true,
            focused: false,
            bounds: [1.0, 2.0, 30.0, 10.0],
            children: vec![],
        }
    }

    #[test]
    fn a_tree_maps_roles_names_states_and_children() {
        let mut slider = leaf(3, Role::Slider, "VOLUME");
        slider.value = Some("7".into());
        let mut tab = leaf(4, Role::Tab, "ONE");
        tab.on = Some(true);
        let mut switch = leaf(5, Role::Switch, "SOUND");
        switch.on = Some(false);
        let mut off = leaf(6, Role::Button, "GO");
        off.enabled = false;
        let mut heading = leaf(7, Role::Heading, "TITLE");
        heading.level = 2;
        let mut text = leaf(8, Role::TextBox, "NAME");
        text.value = Some("ada".into());
        let mut nav = leaf(2, Role::Navigation, "SITE");
        nav.children = vec![leaf(9, Role::Link, "HOME")];
        let u = tree_update(
            &[nav, slider, tab, switch, off, heading, text],
            8,
            [480.0, 300.0],
        );
        let get = |id: u64| &u.nodes.iter().find(|(n, _)| n.0 == id).unwrap().1;
        assert_eq!(get(2).role(), K::Navigation);
        assert_eq!(get(2).label(), Some("SITE"));
        assert_eq!(get(2).children(), [NodeId(9)]);
        assert_eq!(get(9).role(), K::Link);
        assert_eq!(get(3).numeric_value(), Some(7.0));
        assert_eq!(get(4).is_selected(), Some(true));
        assert_eq!(get(5).toggled(), Some(Toggled::False));
        assert!(get(6).is_disabled() && !get(6).supports_action(Action::Click));
        assert!(get(5).supports_action(Action::Click) && get(5).supports_action(Action::Focus));
        assert_eq!(get(7).level(), Some(2));
        assert_eq!(get(8).value(), Some("ada"));
        assert_eq!(
            get(2).bounds(),
            Some(Rect {
                x0: 1.0,
                y0: 2.0,
                x1: 31.0,
                y1: 12.0
            })
        );
        // The window is the root, holds the top level, and the focus is valid.
        assert_eq!(u.tree.as_ref().unwrap().root, ROOT);
        assert_eq!(get(ROOT.0).role(), K::Window);
        assert_eq!(get(ROOT.0).children().len(), 7);
        assert_eq!(u.focus, NodeId(8));
        // A focus that is not in the tree falls back to the window.
        assert_eq!(tree_update(&[], 99, [1.0, 1.0]).focus, ROOT);
    }
}
