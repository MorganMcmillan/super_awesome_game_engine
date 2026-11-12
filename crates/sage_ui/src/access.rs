//! Semantics: what a screen reader or a test sees instead of pixels.
//!
//! Every node may carry an [`AccessInfo`] (a role, a name, a description and
//! a heading level). The widgets imply a role of their own (a button, a
//! slider, a text box), so a screen is accessible before anyone annotates
//! it. [`tree`] derives the accessibility tree from a laid-out [`Tree`]:
//! hidden nodes are left out, plain containers are flattened into their
//! parent, and each node's name is worked out the way the web does it (an
//! explicit name, then the `<label for>` that points at it, then its own
//! text). The result is plain data, so an adapter for a platform's
//! accessibility API can be written against it without touching the UI.
//! [`outline`] writes it as indented text, which is what the tests compare.

use crate::layout::{Node, Tree, Widget};

/// What a node is, in the terms of WAI-ARIA roles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Role {
    /// A container with no meaning of its own; it is left out of the tree.
    #[default]
    Generic = 0,
    Button,
    Checkbox,
    Radio,
    Switch,
    Slider,
    TextBox,
    Link,
    Heading,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    Tab,
    TabList,
    TabPanel,
    Tree,
    TreeItem,
    Menu,
    MenuBar,
    MenuItem,
    Dialog,
    Tooltip,
    ProgressBar,
    Separator,
    Navigation,
    Banner,
    ContentInfo,
    Main,
    Region,
    Form,
    Group,
    Image,
    Text,
    ComboBox,
    ListBox,
    Option,
    RadioGroup,
    Label,
    Application,
}

const ROLES: [(Role, &str); 42] = [
    (Role::Generic, "generic"),
    (Role::Button, "button"),
    (Role::Checkbox, "checkbox"),
    (Role::Radio, "radio"),
    (Role::Switch, "switch"),
    (Role::Slider, "slider"),
    (Role::TextBox, "textbox"),
    (Role::Link, "link"),
    (Role::Heading, "heading"),
    (Role::List, "list"),
    (Role::ListItem, "listitem"),
    (Role::Table, "table"),
    (Role::Row, "row"),
    (Role::Cell, "cell"),
    (Role::ColumnHeader, "columnheader"),
    (Role::Tab, "tab"),
    (Role::TabList, "tablist"),
    (Role::TabPanel, "tabpanel"),
    (Role::Tree, "tree"),
    (Role::TreeItem, "treeitem"),
    (Role::Menu, "menu"),
    (Role::MenuBar, "menubar"),
    (Role::MenuItem, "menuitem"),
    (Role::Dialog, "dialog"),
    (Role::Tooltip, "tooltip"),
    (Role::ProgressBar, "progressbar"),
    (Role::Separator, "separator"),
    (Role::Navigation, "navigation"),
    (Role::Banner, "banner"),
    (Role::ContentInfo, "contentinfo"),
    (Role::Main, "main"),
    (Role::Region, "region"),
    (Role::Form, "form"),
    (Role::Group, "group"),
    (Role::Image, "img"),
    (Role::Text, "text"),
    (Role::ComboBox, "combobox"),
    (Role::ListBox, "listbox"),
    (Role::Option, "option"),
    (Role::RadioGroup, "radiogroup"),
    (Role::Label, "label"),
    (Role::Application, "application"),
];

impl Role {
    pub fn from_u32(v: u32) -> Role {
        ROLES
            .iter()
            .map(|r| r.0)
            .find(|r| *r as u32 == v)
            .unwrap_or(Role::Generic)
    }

    pub fn name(self) -> &'static str {
        ROLES[self as usize].1
    }

    /// The role called `name` (`"button"`, `"navigation"`...), if there is one.
    pub fn from_name(name: &str) -> Option<Role> {
        ROLES.iter().find(|r| r.1 == name).map(|r| r.0)
    }

    /// Does this role take its name from what is inside it (a button's text,
    /// a heading's text) rather than from an attribute?
    fn names_from_content(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::Link
                | Role::Heading
                | Role::ListItem
                | Role::Cell
                | Role::ColumnHeader
                | Role::Tab
                | Role::TreeItem
                | Role::MenuItem
                | Role::Option
                | Role::Tooltip
                | Role::Label
                | Role::Text
        )
    }
}

/// What a node says about itself, apart from what its widget implies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessInfo {
    /// A [`Role`] as a number; 0 keeps the one the widget implies.
    pub role: u32,
    /// A heading's level, 1 to 6 (0 for none).
    pub level: u32,
    /// Left out of the tree even though it is drawn (`aria-hidden`).
    pub hidden: bool,
    /// An explicit name (`aria-label`); empty for none.
    pub name: String,
    pub description: String,
    /// The StableId of a node whose text names this one (`aria-labelledby`);
    /// 0 for none.
    pub labelled_by: u64,
    /// On a label: the StableId of the control it names (`<label for>`).
    pub label_for: u64,
}

/// One node of the accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    pub id: u64,
    pub role: Role,
    pub name: String,
    pub description: String,
    /// A slider's number or a text box's text (never a password's).
    pub value: Option<String>,
    /// Checked (check box, switch, radio) or selected (tab, tree item).
    pub on: Option<bool>,
    pub level: u32,
    pub enabled: bool,
    pub focused: bool,
    /// The box on screen: x, y, width, height.
    pub bounds: [f32; 4],
    pub children: Vec<AccessNode>,
}

/// The role a widget has when nothing says otherwise.
pub fn implied_role(n: &Node) -> Role {
    match n.widget {
        Widget::Button => Role::Button,
        Widget::Toggle => Role::Checkbox,
        Widget::Choice => Role::Checkbox,
        Widget::Slider => Role::Slider,
        Widget::Meter => Role::ProgressBar,
        Widget::TextInput => Role::TextBox,
        Widget::Label => Role::Label,
        Widget::Viewport => Role::Application,
        Widget::Handle => Role::Separator,
        Widget::None | Widget::Block => {
            if n.image.is_some() && n.text.is_empty() {
                Role::Image
            } else if !n.text.is_empty() {
                Role::Text
            } else {
                Role::Generic
            }
        }
    }
}

fn role_of(n: &Node) -> Role {
    match n.access.as_ref().map(|a| Role::from_u32(a.role)) {
        Some(r) if r != Role::Generic => r,
        _ => implied_role(n),
    }
}

/// The text a node shows, with what a password hides left out.
fn own_text(n: &Node) -> String {
    if n.widget == Widget::TextInput {
        return String::new();
    }
    n.text.trim().to_string()
}

/// What the derivation reads over and over: children by parent, which node
/// labels which control, and which nodes are showing.
struct Ctx<'a> {
    tree: &'a Tree,
    kids: Vec<Vec<usize>>,
    /// Control StableId -> the label node that names it.
    labels: std::collections::BTreeMap<u64, usize>,
    by_id: std::collections::BTreeMap<u64, usize>,
    live: Vec<bool>,
}

impl<'a> Ctx<'a> {
    fn new(tree: &'a Tree) -> (Ctx<'a>, Vec<usize>) {
        let mut kids: Vec<Vec<usize>> = vec![vec![]; tree.nodes.len()];
        let mut roots = vec![];
        let mut labels = std::collections::BTreeMap::new();
        let mut by_id = std::collections::BTreeMap::new();
        for (i, n) in tree.nodes.iter().enumerate() {
            by_id.entry(n.id).or_insert(i);
            match n.parent.filter(|&p| p < tree.nodes.len() && p != i) {
                Some(p) => kids[p].push(i),
                None => roots.push(i),
            }
            if let Some(a) = &n.access
                && a.label_for != 0
            {
                labels.entry(a.label_for).or_insert(i);
            }
        }
        // Siblings in the order they are shown.
        for list in &mut kids {
            if list.iter().any(|&k| tree.nodes[k].order != 0) {
                list.sort_by_key(|&k| tree.nodes[k].order);
            }
        }
        let live = tree.live();
        (
            Ctx {
                tree,
                kids,
                labels,
                by_id,
                live,
            },
            roots,
        )
    }

    fn shown(&self, i: usize) -> bool {
        self.live[i] && !self.tree.nodes[i].access.as_ref().is_some_and(|a| a.hidden)
    }

    /// The text inside node `i`: its own, then that of the plain text and
    /// containers below it (not that of controls, which have names of their
    /// own).
    fn content(&self, i: usize, depth: usize) -> String {
        let mut parts = vec![own_text(&self.tree.nodes[i])];
        if depth < 32 {
            for &c in &self.kids[i] {
                if self.shown(c)
                    && matches!(role_of(&self.tree.nodes[c]), Role::Text | Role::Generic)
                {
                    parts.push(self.content(c, depth + 1));
                }
            }
        }
        parts.retain(|p| !p.is_empty());
        parts.join(" ")
    }

    /// The name of node `i`: an explicit one, then the node it is labelled
    /// by, then the `<label for>` that points at it, then (for roles named
    /// by their content) what is inside it, then a text box's placeholder.
    fn name(&self, i: usize, role: Role) -> String {
        let n = &self.tree.nodes[i];
        if let Some(a) = &n.access {
            if !a.name.is_empty() {
                return a.name.clone();
            }
            if let Some(&l) = self
                .by_id
                .get(&a.labelled_by)
                .filter(|_| a.labelled_by != 0)
            {
                let t = self.content(l, 0);
                if !t.is_empty() {
                    return t;
                }
            }
        }
        if let Some(&l) = self.labels.get(&n.id) {
            let t = self.content(l, 0);
            if !t.is_empty() {
                return t;
            }
        }
        if role == Role::TextBox {
            return n.field.placeholder.clone();
        }
        if role.names_from_content() {
            return self.content(i, 0);
        }
        String::new()
    }

    fn describe(&self, i: usize, role: Role, focus: u64) -> AccessNode {
        let n = &self.tree.nodes[i];
        let value = match n.widget {
            Widget::Slider | Widget::Meter => Some(format!("{}", n.value)),
            Widget::TextInput if n.field.masked => Some(String::new()),
            Widget::TextInput => Some(n.text.clone()),
            _ => None,
        };
        let on = matches!(n.widget, Widget::Toggle | Widget::Choice).then_some(n.on);
        let info = n.access.as_ref();
        AccessNode {
            id: n.id,
            role,
            name: self.name(i, role),
            description: info.map(|a| a.description.clone()).unwrap_or_default(),
            value,
            on,
            level: info.map_or(0, |a| a.level),
            enabled: n.enabled,
            focused: n.id == focus,
            bounds: [n.rect.x, n.rect.y, n.rect.w, n.rect.h],
            children: vec![],
        }
    }

    fn walk(&self, i: usize, depth: usize, focus: u64, out: &mut Vec<AccessNode>) {
        if depth > 64 || !self.shown(i) {
            return;
        }
        let role = role_of(&self.tree.nodes[i]);
        let mut inner = vec![];
        for &k in &self.kids[i] {
            self.walk(k, depth + 1, focus, &mut inner);
        }
        // A plain container has no meaning: what is in it goes up a level.
        if role == Role::Generic {
            out.append(&mut inner);
            return;
        }
        let mut node = self.describe(i, role, focus);
        // Text with nothing to say, and a label whose text already names the
        // control it points at, are not things to read.
        if (role == Role::Text && node.name.is_empty() && inner.is_empty())
            || (role == Role::Label
                && self.tree.nodes[i]
                    .access
                    .as_ref()
                    .is_some_and(|a| a.label_for != 0 && self.by_id.contains_key(&a.label_for)))
        {
            out.append(&mut inner);
            return;
        }
        // The text that names a node is not a second thing to read.
        if role.names_from_content() {
            inner.retain(|c| c.role != Role::Text);
        }
        node.children = inner;
        out.push(node);
    }
}

/// The accessibility tree of a laid-out tree: the nodes that have a role,
/// nested the way they are in the UI. `focus` is the StableId that has the
/// keyboard focus.
pub fn tree(tree: &Tree, focus: u64) -> Vec<AccessNode> {
    let (ctx, roots) = Ctx::new(tree);
    let mut out = vec![];
    for r in roots {
        ctx.walk(r, 0, focus, &mut out);
    }
    out
}

/// The tree as indented text, one node a line: the role, the name in quotes,
/// then `value=`, `level=` and the states that apply.
pub fn outline(nodes: &[AccessNode]) -> String {
    fn go(out: &mut String, n: &AccessNode, depth: usize) {
        out.push_str(&"  ".repeat(depth));
        out.push_str(n.role.name());
        if !n.name.is_empty() {
            out.push_str(&format!(" {:?}", n.name));
        }
        if n.level > 0 {
            out.push_str(&format!(" level={}", n.level));
        }
        if let Some(v) = &n.value {
            out.push_str(&format!(" value={v:?}"));
        }
        match n.on {
            Some(true) => out.push_str(" [on]"),
            Some(false) => out.push_str(" [off]"),
            None => {}
        }
        if n.focused {
            out.push_str(" [focused]");
        }
        if !n.enabled {
            out.push_str(" [disabled]");
        }
        if !n.description.is_empty() {
            out.push_str(&format!(" desc={:?}", n.description));
        }
        out.push('\n');
        for c in &n.children {
            go(out, c, depth + 1);
        }
    }
    let mut out = String::new();
    for n in nodes {
        go(&mut out, n, 0);
    }
    out
}

/// The node with this StableId, anywhere in the tree.
pub fn find(nodes: &[AccessNode], id: u64) -> Option<&AccessNode> {
    nodes.iter().find_map(|n| {
        if n.id == id {
            Some(n)
        } else {
            find(&n.children, id)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{FieldNode, Node, Size, layout};

    /// A 40x10 node under the node at index `parent`.
    fn node(id: u64, parent: usize, widget: Widget, text: &str) -> Node {
        Node {
            id,
            parent: Some(parent),
            width: Size::Fixed(40.0),
            height: Size::Fixed(10.0),
            widget,
            text: text.into(),
            ..Node::default()
        }
    }

    fn info(role: Role) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: role as u32,
            ..AccessInfo::default()
        })
    }

    fn laid(nodes: Vec<Node>) -> Tree {
        let mut t = Tree { nodes };
        layout(&mut t, [400.0, 400.0]);
        t
    }

    #[test]
    fn widgets_imply_roles_and_containers_without_meaning_are_flattened() {
        let root = Node {
            id: 1,
            width: Size::Fixed(300.0),
            height: Size::Fixed(300.0),
            ..Node::default()
        };
        let mut slider = node(5, 0, Widget::Slider, "");
        slider.value = 3.0;
        let mut field = node(6, 0, Widget::TextInput, "ada");
        field.field = FieldNode {
            placeholder: "YOUR NAME".into(),
            ..FieldNode::default()
        };
        let mut toggle = node(7, 0, Widget::Toggle, "");
        toggle.on = true;
        let mut pass = node(8, 0, Widget::TextInput, "hunter2");
        pass.field.masked = true;
        let mut off = node(9, 0, Widget::Button, "GONE");
        off.hidden = true;
        let t = laid(vec![
            root,
            node(2, 0, Widget::Button, "SAVE"),
            // A plain container holding a button: the button moves up.
            node(3, 0, Widget::None, ""),
            node(4, 2, Widget::Button, "CANCEL"),
            slider,
            field,
            toggle,
            pass,
            off,
        ]);
        let out = tree(&t, 6);
        // The hidden button is gone, the container is flattened, and the
        // password's text is never exposed.
        assert_eq!(
            outline(&out),
            "button \"SAVE\"\n\
             button \"CANCEL\"\n\
             slider value=\"3\"\n\
             textbox \"YOUR NAME\" value=\"ada\" [focused]\n\
             checkbox [on]\n\
             textbox value=\"\"\n"
        );
    }

    #[test]
    fn explicit_roles_names_and_levels_win_and_hidden_nodes_are_left_out() {
        let mut nav = node(2, 0, Widget::None, "");
        nav.access = Some(AccessInfo {
            role: Role::Navigation as u32,
            name: "MAIN MENU".into(),
            description: "THE SITE'S SECTIONS".into(),
            ..AccessInfo::default()
        });
        let mut title = node(3, 1, Widget::None, "WELCOME");
        title.access = Some(AccessInfo {
            role: Role::Heading as u32,
            level: 2,
            ..AccessInfo::default()
        });
        let mut named = node(4, 1, Widget::Button, "X");
        named.access = Some(AccessInfo {
            name: "CLOSE".into(),
            ..AccessInfo::default()
        });
        let mut decoration = node(5, 1, Widget::None, "***");
        decoration.access = Some(AccessInfo {
            hidden: true,
            ..AccessInfo::default()
        });
        let mut disabled = node(6, 1, Widget::Button, "OFF");
        disabled.enabled = false;
        let t = laid(vec![
            Node {
                id: 1,
                width: Size::Fixed(200.0),
                height: Size::Fixed(200.0),
                ..Node::default()
            },
            nav,
            title,
            named,
            decoration,
            disabled,
        ]);
        assert_eq!(
            outline(&tree(&t, 0)),
            "navigation \"MAIN MENU\" desc=\"THE SITE'S SECTIONS\"\n  \
               heading \"WELCOME\" level=2\n  \
               button \"CLOSE\"\n  \
               button \"OFF\" [disabled]\n"
        );
    }

    #[test]
    fn a_label_for_names_its_control_and_is_not_read_twice() {
        let mut label = node(2, 0, Widget::Label, "YOUR NAME");
        label.access = Some(AccessInfo {
            label_for: 3,
            ..AccessInfo::default()
        });
        let field = node(3, 0, Widget::TextInput, "");
        let mut by = node(4, 0, Widget::Slider, "");
        by.access = Some(AccessInfo {
            labelled_by: 5,
            ..AccessInfo::default()
        });
        let heading = node(5, 0, Widget::None, "VOLUME");
        let t = laid(vec![
            Node {
                id: 1,
                width: Size::Fixed(200.0),
                height: Size::Fixed(200.0),
                ..Node::default()
            },
            label,
            field,
            by,
            heading,
        ]);
        let out = tree(&t, 0);
        assert_eq!(
            outline(&out),
            "textbox \"YOUR NAME\" value=\"\"\nslider \"VOLUME\" value=\"0\"\ntext \"VOLUME\"\n"
        );
        assert_eq!(find(&out, 3).unwrap().name, "YOUR NAME");
        assert!(find(&out, 99).is_none());
    }

    #[test]
    fn a_role_is_named_by_the_text_nested_inside_it_and_that_text_is_not_a_node() {
        let mut row = node(2, 0, Widget::None, "");
        row.access = info(Role::ListItem);
        let inner = node(3, 1, Widget::None, "FIRST");
        let wrapper = node(4, 1, Widget::None, "");
        let deep = node(5, 3, Widget::None, "ITEM");
        let t = laid(vec![
            Node {
                id: 1,
                width: Size::Fixed(200.0),
                height: Size::Fixed(200.0),
                ..Node::default()
            },
            row,
            inner,
            wrapper,
            deep,
        ]);
        assert_eq!(outline(&tree(&t, 0)), "listitem \"FIRST ITEM\"\n");
    }

    #[test]
    fn role_names_round_trip_and_unknown_numbers_are_generic() {
        for v in 0..42 {
            let r = Role::from_u32(v);
            assert_eq!(r as u32, v);
            assert_eq!(Role::from_name(r.name()), Some(r));
        }
        assert_eq!(Role::from_u32(99), Role::Generic);
        assert_eq!(Role::from_name("nope"), None);
    }
}
