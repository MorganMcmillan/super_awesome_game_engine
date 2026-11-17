# UI and UX lessons from professional creative tools (Photoshop, Blender, Figma and others), as of November 2026

Scope note: these notes are from a limited research pass (about 18 tool calls). Every claim below has a source link. Topics that could not be checked against a source are listed under Gaps, not stated as fact. Search-engine summaries were checked against the primary pages where possible. Where only a secondary summary was available, the finding says so.

## Photoshop: non-destructive editing, History, tool options, contextual task bar, in-app learning, generative UI

### Takeaway
Photoshop uses two separate safety nets. One lives only for the session: the History panel, with a 50-state default and snapshots. The other is saved with the document: layers, generative layers and the other non-destructive layer types. Adobe's recent UI work puts the right action in front of the user: an on-canvas contextual task bar, a searchable Discover panel, and generative features attached to selections. Adobe's own design write-ups stress fewer steps and plain, action-oriented language for novices.

### Cited Findings
- By default the History panel keeps the last 50 steps. The count can be changed in Preferences > Performance, and more states cost memory. — [Adobe: History panel overview](https://helpx.adobe.com/photoshop/desktop/get-started/set-up-toolbars-panels/history-panel-overview.html); [Photography Life: how to undo/redo](https://photographylife.com/how-to-undo-redo-in-photoshop)
- Adobe calls snapshots "temporary copies of your image at different editing stages", available in the History panel until the document is closed. — [Adobe: Create work snapshots](https://helpx.adobe.com/photoshop/desktop/get-started/set-up-toolbars-panels/create-work-snapshots.html). Some third-party tutorials disagree on whether snapshots survive closing the file; Adobe's wording (they last only for the session) is the more reliable. — [Adobe: Manage image states](https://helpx.adobe.com/nz/photoshop/desktop/get-started/set-up-toolbars-panels/manage-image-states.html)
- "Allow Non-Linear History" lets the user discard or edit an earlier history state without deleting the states after it. — [Adobe: Manage image states](https://helpx.adobe.com/nz/photoshop/desktop/get-started/set-up-toolbars-panels/manage-image-states.html)
- History and snapshots live in working memory and are not saved with the file. That is why adjustment layers and masks matter for keeping edits reversible later. — [Julieanne Kost (Adobe): Undo, History panel, History brushes](https://jkost.com/blog/2021/02/working-with-undo-the-history-panel-history-and-art-history-brushes-in-photoshop.html) (secondary summary of the page)
- The contextual task bar is a floating, on-canvas menu that shows the next likely actions for the active tool or selection, for example Select Subject and Remove Background, so users do not have to search panels and menus. It is on by default and can be docked, floated or hidden. Later releases added text alignment, leading and kerning, and font style to it. — [Adobe: Contextual task bar](https://helpx.adobe.com/photoshop/using/contextual-task-bar.html); [Adobe: Boost workflows with the contextual task bar](https://helpx.adobe.com/sa_en/photoshop/desktop/get-started/learn-the-basics/boost-workflows-with-the-contextual-task-bar.html)
- Photoshop Elements 2026 added its own contextual task bar, which can be hidden from Window > Contextual Task Bar. — [Adobe: Photoshop Elements contextual task bar](https://helpx.adobe.com/tr/photoshop-elements/desktop/workspace-and-environment/contextual-task-bar.html)
- The Discover panel is Photoshop's searchable help and learning panel. It opens from the search icon, Ctrl/Cmd+F, or Help > Photoshop Help. — [Adobe: Workspace basics](https://helpx.adobe.com/photoshop/using/workspace-basics.html)
- Experts complain that Discover pops up over their work. The workaround they share is to rebind its shortcut; the threads show no official way to turn it off. — [Adobe Community: How do I disable the Discover panel?](https://community.adobe.com/t5/photoshop-ecosystem-discussions/how-do-i-disable-the-discover-panel/m-p/13804363)
- At launch (May 2023), Adobe said Generative Fill is "infused into every selection feature" and that "a new generative layer type" lets people work non-destructively. — [Adobe Blog, 23 May 2023](https://blog.adobe.com/en/publish/2023/05/23/photoshop-new-features-ai-contextual-presets)
- Adobe Design's "Behind the design: Reference Image" (undated) describes four rounds of research with professional and non-professional creatives on "Protoshop", a web prototype that could be changed in hours or days instead of weeks. Findings:
  - Users were confused about how the reference image related to the prompt.
  - A continuous influence slider was reduced to a two-position "Reference match" control (Low/High).
  - The term "generative compositing" confused users, and layers slowed newcomers down, so the team moved to action-oriented wording.
  - Users expected the reference to be copied exactly, not just to guide the result.
  - Lessons: "adapt the technology to user behavior rather than expecting users to conform to it", and extra steps and options made the experience feel intimidating.

  — [Adobe Design: Behind the design, Photoshop Reference Image](https://adobe.design/ideas/behind-the-design-adobe-photoshop-reference-image)
- A second Adobe Design piece covers letting users pick third-party ("partner") generative models inside Photoshop. — [Adobe Design: Partner models in Photoshop](https://adobe.design/ideas/behind-the-design-partner-models-in-adobe-photoshop) (not fetched in full)

### Inferences
- Photoshop keeps "undo the last action" (session History) separate from "change a decision later" (layers saved with the document). For a game editor: undo history can be short-lived, but anything a user may want to change later should live in the saved scene as editable data, such as prefab overrides, components and modifier-like systems, not in the undo log.
- The contextual task bar is a "next likely action" surface for novices that experts can hide. The Discover complaints show what happens when help surfaces cannot be dismissed: teaching aids must be easy to turn off.
- The Reference Image research suggests that continuous parameters, which experts like, may need simplified versions (presets, two-state toggles) at the beginner level, and that labels should describe outcomes ("swap", "place") rather than mechanisms ("compositing").

### Gaps
- No primary Adobe source was fetched on smart objects or adjustment layers as a design rationale. Those are well documented in help pages, but this pass did not verify the specific claims.
- No Adobe write-up on the tool palette plus tool options bar, or on workspaces as design decisions, was found in the time available.
- No hard data (studies, telemetry) on whether the contextual task bar or Discover improved novice success was found.

## Blender 2.80 (2019) UI overhaul: left-click select, toolbar, workspaces, search, Quick Favorites, keymaps; procedural editing and scripting

### Takeaway
Blender 2.80 (released July 2019) aimed at both levels on purpose. It added visible, mouse-first ways in: a left-click select default, a persistent tool toolbar with settings in a top bar, task-based workspace tabs, right-click context menus, and an "Adjust Last Operation" panel. It kept and extended the keyboard paths experts use: three selectable keymaps, pie menus, Quick Favorites, and menu search that also finds operators not in any menu. The design document and the developers state that the visual paths are additions to shortcuts, not replacements.

### Cited Findings
- 2.80 made left-click the default select button. Right-click opens a context menu, Shift/Ctrl extend or remove from the selection, and dragging with the default Box Select tool box-selects. Right-click select stays supported "for its efficiency and health advantages when using a keyboard and mouse". The change also supports one-button trackpads and pen input. — [Blender 2.80 release notes: User Interface](https://developer.blender.org/docs/release_notes/2.80/ui/)
- The 2.80 notes describe the new viewport toolbar and settings:
  - The toolbar holds tools that stay active until the user switches tool.
  - Tool settings appear in a bar at the top of the window, and detailed settings (for example sculpt brushes) appear in the Tool tab of the Properties editor.

  — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/)
- The 2.80 notes describe workspaces as follows:
  - Workspaces are tabs at the top of the window with layouts for tasks such as modeling or sculpting.
  - Each workspace switches to its object mode automatically.
  - Workspaces can be customized, duplicated, appended from other .blend files, or deleted.

  — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/). The manual adds Ctrl+PageUp/PageDown to cycle workspaces and double-click to rename. — [Blender 2.81 manual: Workspaces](https://docs.blender.org/manual/en/2.81/interface/window_system/workspaces.html)
- Quick Favorites (Q) is a user-built popup menu; items are added by right-clicking any button or menu entry. — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/)
- 2.80 ships three keymaps: Blender (default), Blender 2.7 (closest to before) and Industry Compatible (shortcuts from other 3D apps). The stated reason is that "the number of available shortcut keys for new features and users to map their own has become too small." — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/)
- Pie menus (orientation, pivot, shading, snap and others) are meant to "make more efficient use of the keyboard to group related features". — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/)
- The 2.80 notes list other changes:
  - an "Adjust Last Operation" panel in the corner of each editor;
  - a status bar showing what the mouse and keys do, progress and scene statistics;
  - property "decorators" showing animated, keyframed, driven, linked and overridden state;
  - theme-coloured icons.

  — [Blender 2.80 release notes: UI](https://developer.blender.org/docs/release_notes/2.80/ui/)
- Menu Search (F3) searches the whole interface for an operator and runs it, and shows the menu path where the operator lives. It "searches all Operators within Blender, even if they are not exposed in a menu. This is useful for Python developers." — [Blender manual: Menu Search / Operator Search](https://docs.blender.org/manual/en/3.3/interface/controls/templates/operator_search.html)
- The Blender 2.8 Design Document (Dalai Felinto, 7 October 2017) was signed off by the core team, including Pablo Vazquez, Julian Eisel, Campbell Barton and Ton Roosendaal. It says:
  - "Emphasis is on helping specialists or to enable specialist tasks better."
  - Its goal is to "optimise one's personal workflow better, instead of trying to maintain a single configuration."
  - "Workspaces allow users to build up a working environment for a specific task", and the workspace "stores a mode, instead of having it per object".
  - "All interactive elements of a manipulator should be visible (e.g., not only on cursor hover)."
  - It proposes "Blender 101" reduced interfaces for fields such as 3D printing and previsualization.
  - In the comments, Felinto says the new non-shortcut options are an addition to shortcuts, not a replacement.

  — [code.blender.org: Blender 2.8 Design Document](https://code.blender.org/2017/10/blender-2-8-design-document/)
- William Reynish, "Tools, Toolbar and Tool Widgets" (28 April 2018), says the 2.79 toolbar operators were modal, so clicking Translate started reading cursor movement and the object could not be moved left. The post gives these goals:
  - consistent tools across modes, and a slimmer toolbar with icon-only, two-column or icon-plus-text layouts;
  - tool settings moved to the top bar so they stay visible when the toolbar is hidden;
  - Blender should be "used in a more visual way, not just via shortcuts" while keeping "Blender's efficient keyboard-focused workflow".

  Campbell Barton commented that "Changing tools via a toolbar isn't efficient for experienced users". Many commenters doubted expert speed would survive, so the tension was openly unresolved at the time. — [code.blender.org: Tools, Toolbar and Tool Widgets](https://code.blender.org/2018/04/tools-toolbar-and-tool-widgets/)
- Reynish had published a UI analysis of Blender as early as 2008 (Blender Conference paper, later updated as a 25-page PDF). — [BlenderNation, 2008](https://www.blendernation.com/2008/11/12/william-reynish-ui-paper/)

### Inferences
- Blender's pattern is "visible way in, keyboard way out":
  - Every visible control also has a shortcut, appears in F3 search, and can be added to Quick Favorites with a right-click.
  - Each layer is one step faster than the last, and the UI itself teaches the faster path: the status bar shows key hints, menus show shortcuts, and search shows menu paths.
- "Adjust Last Operation" turns a destructive one-shot operator into a briefly editable one: the user tweaks its parameters right after running it. That is cheap non-destructive editing without a full modifier stack, and it suits a game editor where operations like "spawn grid of entities" take parameters.
- Shipping several keymaps, including an industry-compatible one, lowers the cost of switching tools for people coming from Unity, Godot or other 3D apps.
- The release notes' property decorators (animated, driven, linked, overridden) are a direct precedent for showing in an inspector whether a component field is inherited from a prefab, overridden, or driven by a system or script.

### Gaps
- Drivers, the modifier stack, Geometry Nodes, and Python tooltips (hovering a property shows its `bpy` path when the "Python Tooltips" preference is on) were not verified from a primary source in this pass. They are well documented in the Blender manual and should be checked there before being cited.
- No primary source was found for the claim that 2.80 "widened adoption". User-growth numbers, such as download counts or Blender Development Fund growth after 2019, were not found in this pass.
- No Pablo Vazquez talk on UI was located. He is confirmed only as a signatory of the 2.8 design document.

## Figma: components, instances and overrides, multiplayer, version history, browser-first

### Takeaway
A Figma instance is a linked copy that takes every property from its main component unless overridden. Overrides are kept per instance, even when the instance is swapped or its variant changes. In 2026 Figma rebuilt instance resolution as a general "derived subtree" system with fine-grained, push-based invalidation, because deep nesting caused cascading updates. Multiplayer is server-authoritative last-writer-wins per property, with a stated rule for undo.

### Cited Findings
- Naomi Jung, "How we rebuilt the foundations of component instances", Figma Blog, 17 March 2026. Main points:
  - Instances are linked copies that resolve properties from the main component: "When a user edits a main component, its instances will automatically stay in sync."
  - Instances can stack variants, variable bindings and auto layout.
  - The old Instance Updater re-resolved an entire instance on any change, which caused recursive cascades in deeply nested components and froze the editor. Changing a variable mode, for example, could re-lay out thousands of nodes, and systems "fought" over the same subtree.
  - The new "Materializer" builds "derived subtrees" from a blueprint saying what the instance inherits, which overrides apply and which children exist. Dependencies are recorded automatically as nodes read data, and changes are pushed to dependents, which are marked dirty and recomputed later.
  - Layout and variables stay in their own systems, coordinated one way.
  - Variable mode changes in large files got 40–50% faster in internal benchmarks. The extra dependency caches cost load time and memory.
  - The same foundation now supports CMS-synced rich text and "slots".

  — [Figma Blog, 17 March 2026](https://www.figma.com/blog/how-we-rebuilt-the-foundations-of-component-instances/)
- Figma keeps a user's overrides on an instance when the instance is swapped or a different variant is chosen. Text overrides carry over when the text layers in both components have the same name. — Figma Learn, reported via search summary: [Edit instances with component properties](https://figma-signup.helpjuice.com/use-libraries/edit-instances-with-component-properties) (mirror; not fetched directly); [10 tips on using components in Figma](https://medium.com/design-with-figma/10-tips-on-using-components-in-figma-c7db9c5e7fe1)
- Overrides apply to one instance only and do not change the main component or other instances. Overridable properties include text, visibility and nested icon swaps. — [LogRocket blog](https://blog.logrocket.com/?p=155507) (secondary)
- Users report that overrides on nested instances become rigid or reset unpredictably. These are forum reports, not confirmed bugs. — [Figma Forum: overrides on instances inside instances get reset](https://forum.figma.com/report-a-problem-6/overrides-on-instances-inside-instances-get-reset-inconsistantly-58140); [Figma Forum: how does inheritance work](https://forum.figma.com/ask-the-community-7/instanced-components-how-does-inheritance-work-can-t-figure-out-a-supposed-bug-34201)
- Components launched in December 2016. — [The Next Web, 8 December 2016](https://thenextweb.com/news/figma-introduces-components-make-editing-multiple-design-elements-incredibly-easy)
- Evan Wallace, "How Figma's multiplayer technology works" (16 October 2019):
  - Clients are web pages connected over WebSockets, with one server process per document, and the server sets the order of events.
  - The document is effectively a map from object ID to a property map, and the server keeps the latest value per (object, property). Edits to different properties or different objects never conflict, and a text value is replaced whole, not merged.
  - Clients discard incoming changes that conflict with their own unacknowledged edits, to avoid flicker.
  - Clients mint object IDs that include a client ID, so objects can be created offline.
  - The parent link and fractional-index position are stored together as one property, and the server rejects parenting cycles.
  - Deleted objects' data is kept in the deleting client's undo buffer.
  - Undo rule: undoing a lot, copying something, then redoing back to the present should leave the document unchanged. Undo rewrites the redo history at the time of the undo, so a redo does not overwrite other people's later edits.
  - Figma's own summary: it is "inspired by" CRDTs but not a true CRDT.

  — [Evan Wallace: How Figma's multiplayer technology works](https://madebyevan.com/figma/how-figmas-multiplayer-technology-works/) (archived copy of the Figma Blog post); [Figma Blog original](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)
- Fractional indexing has been used for ordered children in Figma multiplayer "from the beginning". — [Figma Blog: Realtime editing of ordered sequences](https://www.figma.com/blog/realtime-editing-of-ordered-sequences/)

### Inferences
- Figma's main component and instance model maps directly onto ECS prefabs: in Flecs, `IsA` plus `OnInstantiate Inherit`/`Override`. The project already uses `Inherit` for UI style groups, so editing a style prefab restyles its instances. Lessons that carry over:
  - Store overrides as sparse data per instance, keyed by a stable path, not by position.
  - Keep overrides when the base is swapped, matching by name or StableId.
  - Expect nested-override behaviour to be the hardest part to explain to users.
- Figma's 2026 rebuild warns about deep nesting: naive "re-resolve the whole instance" propagation does not scale. Dirty tracking on changed properties is needed once prefabs nest.
- Property-level last-writer-wins with one authority fits the project's scene format, where a scene is a set of entities with component fields. The rule that "redo doesn't overwrite others' later edits" applies even with one user, when a play-in-editor or live-reload process also writes to the document.
- Instance overrides are a form of non-destructive editing that sits in the document. The Photoshop-style split from the first section applies: overrides are saved state, not undo history.

### Gaps
- Figma's own writing on auto layout, the properties panel's design, the plugin API's sandbox design, version history, and multiplayer cursors/presence (Wallace's post leaves presence out) was not fetched in this pass.
- No evidence was gathered on why browser-first helped adoption (sharing by link, no install), only common claims.

## Other tools: Ableton Live, DaVinci Resolve, Houdini-style node graphs, Krita, Procreate

### Takeaway
Ableton's Session View started as a live improvisation tool built in Max by the founders, not as a recording interface. DaVinci Resolve splits work into task pages that all share one timeline. Houdini's node networks make every modelling step editable later, but users find this harder to learn than direct modelling and often mix the two. Krita and Procreate could not be sourced in this pass.

### Cited Findings
- Gerhard Behles, Robert Henke and Bernd Roggendorf started Live in the mid-1990s. Many of version 1's ideas came from Max patches they had built for their own music. Henke: "The main goal was to build a system that allowed us to improvise with material on stage." Behles thinks, but is not sure, that Henke brought the first Session View patch. — [MusicRadar: 20 years of Ableton Live](https://www.musicradar.com/news/story-of-ableton-live-at-20); [zplane interview with Gerhard Behles](https://products.zplane.de/blog/gerhard-behles-interview-ableton)
- Session View lets users trigger audio clips that are time-stretched to the project tempo, loop them, and combine sections, such as drums from one part with chords from another. Behles calls it a break from playing songs linearly. — [zplane interview with Gerhard Behles](https://products.zplane.de/blog/gerhard-behles-interview-ableton); [Wikipedia: Ableton Live](https://en.wikipedia.org/wiki/Ableton_Live) (secondary)
- DaVinci Resolve's pages are Media, Cut, Edit, Fusion, Color, Fairlight and Deliver, roughly in film-production order, but users can move between them freely. A timeline built on Edit also appears on Cut, Color and Fairlight, and Deliver renders from it. The Cut page is a fast first-assembly editor, and Edit is the conventional timeline. — [TourBox: DaVinci Resolve pages explained](https://www.tourboxtech.com/en/news/davinci-resolve-pages-explained.html); [CreativeLive (Casey Faris): Resolve is made of pages](https://phoenix.creativelive.com/class/davinci-resolve-how-to-use-every-page-casey-faris/lessons/resolve-is-made-of-pages) (tutorial sources; official Blackmagic documentation not fetched)
- In Houdini, "every step of the modeling process is editable, repeatable and easy to adjust at any time", and each operation becomes a node. — [School of Motion: Houdini procedural workflow](https://www.schoolofmotion.com/blog/houdini-procedural-workflow). SideFX markets the "non-destructive workflow" as giving "control at every stage of the process". — [SideFX: Houdini for Film/TV](https://download.sidefx.com/filmtv/). The procedural approach goes back to PRISMS (1987). — [ModDB: Go procedural](https://www.moddb.com/engines/houdini/news/go-procedural-a-better-way-to-make-better-games)
- On SideFX's own forum, users say procedural, non-destructive modelling is "much harder to understand in actual practice" than direct sculpting and box modelling. Many build a procedural base and then edit destructively on top, "baking" history with a locked null node. — [SideFX forum: modeling module discussion](https://www.sidefx.com/forum/post/144434/)
- SideFX's 2026 community post on the procedural buildings in *The Finals* is an example of procedural tooling in game production. — [SideFX: Making the procedural buildings of The Finals (11 February 2026)](https://www.sidefx.com/community/making-the-procedural-buildings-of-the-finals-using-houdini/) (not fetched)

### Inferences
- Ableton's "liveness" is the closest creative-tool parallel to a deterministic game simulation with live reload: change material while it plays, aligned to a clock. For this engine, tick-aligned hot reload and rewind play the role of tempo-aligned clip launching. Ableton is also a tool its authors built for their own performances first, which is a reason to dogfood with real small games.
- Resolve's model of many task pages over one shared document maps onto editor workspaces (Scene, Blocks, Code, Play/Debug) that all view one world or document. It also backs the project's stated idea that the block editor is a view of the same Python, not a separate asset.
- Houdini's experience is a warning for the expert ceiling: fully procedural editing is powerful but hard to learn. Offering a "bake/apply" escape hatch, as Blender modifiers and Houdini locked nodes do, lets users mix the two styles.

### Gaps
- Krita and Procreate were not researched in this pass; no sources were collected on Krita's brush engine or docker design, or on Procreate's touch gestures (such as two-finger undo) and QuickMenu. Those claims should not be included without sources.
- Substance Designer was not researched.
- No HCI literature (for example on "liveness" levels, or on novice-to-expert transitions in feature-rich software) was fetched in this pass.

## Cross-cutting patterns: non-destructive vs destructive, instance/override, undo granularity, expert keyboard paths, novice discoverability

### Takeaway
The tools above share five patterns:
- They save editable decisions in the document (layers, modifiers, nodes, overrides) and keep undo history short-lived.
- They resolve instances from a base plus sparse overrides.
- They make undo operation-level, collaboration-safe, and sometimes non-linear.
- They layer the ways to reach every command: visible tool, then menu with its shortcut shown, then search, then favourites and pies, then a custom keymap.
- They teach in context: context menus, contextual bars, status-bar key hints, and adjust-last-operation panels.

### Cited Findings
- Session-only history alongside saved non-destructive state: Photoshop's History and snapshots are not saved with the file, while generative layers and other layers are. — [Adobe: Create work snapshots](https://helpx.adobe.com/photoshop/desktop/get-started/set-up-toolbars-panels/create-work-snapshots.html); [Adobe Blog, May 2023](https://blog.adobe.com/en/publish/2023/05/23/photoshop-new-features-ai-contextual-presets)
- Non-linear history (editing or discarding earlier states without losing later ones). — [Adobe: Manage image states](https://helpx.adobe.com/nz/photoshop/desktop/get-started/set-up-toolbars-panels/manage-image-states.html)
- Collaboration-safe undo rule. — [Evan Wallace / Figma, 2019](https://madebyevan.com/figma/how-figmas-multiplayer-technology-works/)
- Fixing a parameter right after the action (Adjust Last Operation); property-state decorators (animated, driven, linked, overridden). — [Blender 2.80 UI release notes](https://developer.blender.org/docs/release_notes/2.80/ui/)
- Layered command access: toolbar, then context menu, then F3 search including commands not in any menu, then Quick Favorites, then pie menus, then a choice of keymaps. — [Blender 2.80 UI release notes](https://developer.blender.org/docs/release_notes/2.80/ui/); [Blender manual: Menu Search](https://docs.blender.org/manual/en/3.3/interface/controls/templates/operator_search.html)
- Visible-by-default affordances: "All interactive elements of a manipulator should be visible (e.g., not only on cursor hover)". — [Blender 2.8 Design Document](https://code.blender.org/2017/10/blender-2-8-design-document/)
- Contextual next action on the canvas. — [Adobe: Contextual task bar](https://helpx.adobe.com/photoshop/using/contextual-task-bar.html)
- Novices are put off by extra options and jargon. — [Adobe Design: Reference Image](https://adobe.design/ideas/behind-the-design-adobe-photoshop-reference-image)
- Experts resent help surfaces they cannot dismiss. — [Adobe Community: disable Discover panel](https://community.adobe.com/t5/photoshop-ecosystem-discussions/how-do-i-disable-the-discover-panel/m-p/13804363)
- Instance resolution must scale with nesting. — [Figma Blog, March 2026](https://www.figma.com/blog/how-we-rebuilt-the-foundations-of-component-instances/)

### Inferences
- Undo granularity: Photoshop counts states per operation, and Figma treats one property change as atomic. One undo step should be one user-intended operation, for example a whole drag rather than each frame of it, stored as a scene difference. The project's `sage_editor::history` already stores undo as scene differences.
- The project already plans rewind and deterministic replay for simulation time. That is a "history of the running game", separate from "history of the authored document". The tools above keep their kinds of history separate: Photoshop's session History vs saved layers, Figma's per-user undo vs shared document. The editor should likewise show "undo edit" and "rewind tick" as distinct controls.

### Gaps
- No controlled studies comparing novice performance with and without these patterns were found in this pass.

## Implications for a game engine editor at each skill level (beginner, intermediate, expert)

### Takeaway
Build one document model: entities, components, prefabs with sparse overrides, and Python. Put several views on top of it, as Resolve does with pages and Blender with workspaces. Layer access to every command so each level has a natural way in and a visible path to the next level, following Blender 2.80. Keep editable decisions in the saved scene and undo or rewind in session history, following Photoshop and Figma.

### Cited Findings
- Precedents for every recommendation below are cited in the sections above: [Blender 2.80 UI notes](https://developer.blender.org/docs/release_notes/2.80/ui/), [Blender 2.8 Design Document](https://code.blender.org/2017/10/blender-2-8-design-document/), [Figma instances rebuild, 2026](https://www.figma.com/blog/how-we-rebuilt-the-foundations-of-component-instances/), [Figma multiplayer, 2019](https://madebyevan.com/figma/how-figmas-multiplayer-technology-works/), [Adobe contextual task bar](https://helpx.adobe.com/photoshop/using/contextual-task-bar.html), [Adobe Design: Reference Image](https://adobe.design/ideas/behind-the-design-adobe-photoshop-reference-image), [Resolve pages (TourBox)](https://www.tourboxtech.com/en/news/davinci-resolve-pages-explained.html), [Houdini forum on procedural difficulty](https://www.sidefx.com/forum/post/144434/).

### Inferences

**Beginner (low floor)**
- Show a few task workspaces as tabs that all view the same world, as Blender and Resolve do: for example Scene, Blocks and Play. A workspace can also set a mode, the way a Blender workspace sets the object mode.
- Use left-click select and visible tools in a toolbar, with tool options in a top bar. Gizmos should always be visible, not only on hover.
- Add a contextual "next action" bar near the selection, with actions like "Add component", "Make prefab", "Add script" and "Play from here". Like Photoshop's, it should be on by default, easy to hide, and never pop up uninvited, which is the Discover complaint.
- Use outcome-language labels ("Make copies follow this one", "Reset to prefab") rather than mechanisms ("IsA", "override"), as Adobe's research found. Use presets or two-state controls before continuous parameters.
- Make undo forgiving: one undo step per intended action. Keep rewind visibly separate from undo.
- Use prefabs as Figma-style components. Editing the prefab updates every instance; changing an instance creates an override, marked in the inspector, with a right-click "Reset to prefab". Blender's decorators show the same kinds of state.

**Intermediate**
- Make every inspector field show its state with a decorator: inherited, overridden, driven by a system or script, or keyed or recorded.
- Allow overrides on nested prefabs, and keep them when the base or variant is swapped, matching by StableId or name as Figma does. Show where a value comes from ("this value comes from prefab X > Y").
- Add an "Adjust last operation" panel for parameterized editor operations such as spawn, duplicate or array.
- Let right-click on any control offer "Add to Quick Favorites", "Copy Python path" and "Show shortcut", Blender's way of moving users from the visible UI to the faster one.
- Treat the block editor as a view of Python, like a Resolve page over one shared timeline. Switching between blocks and text loses nothing, so learners can move from blocks to code at their own pace.

**Expert (high ceiling)**
- Add F3-style search over every editor command, including commands not in any menu, with the menu path and shortcut shown.
- Offer pie menus, rebindable keymaps, and an "industry compatible" keymap preset for Unity and Godot users.
- Make everything scriptable: every editor command should be a Python call that can be found from its tooltip (an analogue of Blender's Python tooltips, which this pass did not verify) and recorded.
- Offer procedural, non-destructive systems in the style of Houdini nodes or Blender modifiers. Always provide "apply/bake", because fully procedural workflows are harder to learn and experts mix procedural and direct editing.
- Make prefab resolution scale: sparse override storage, dirty tracking per changed field, and no whole-instance re-resolve. Figma had to rebuild its instance system for this in 2026. For Flecs, this maps to choosing `OnInstantiate Inherit` (shared and live-updating) vs `Override` (copied) per component, and showing that choice in the UI.
- If multiplayer editing arrives later, Figma's server-ordered last-writer-wins per (entity, component field) is a proven, simple model, and it fits a scene format that is already keyed by StableId.
- For liveness, Ableton-style changes aligned to a clock map to tick-aligned hot reload and rewind-and-replay.

### Gaps
- Mapping Figma's overrides to Flecs `OnInstantiate` traits is an inference. Edge cases such as removing a component on an instance, or reordering children under `ChildOf`, need checking against the pinned Flecs (4.1.2) and the design doc's prefab sections.
- Beginner usability claims should be validated with users. Adobe's four-round prototype method (Protoshop) is a model for doing so cheaply.
