# Game engine editor UX: what helps and what hurts users

Compiled 2026-11-14. The sources are vendor blogs and documentation, developer posts, one peer-reviewed HCI study, and small student studies. Forum and Reddit material is marked **[anecdotal]**. Version numbers and dates are given wherever the source states them. Each claim has an inline source. Anything that could not be sourced is listed under Gaps.

## 1. Unity, Unreal, Godot: what vendors and users say works and what doesn't

### Takeaway
The biggest documented redesigns in the major engines responded to two kinds of complaint:
- **Edits that are unsafe or invisible.** Unity's prefab "Apply" pushed changes with no preview of their effect. Play-mode edits are silently discarded.
- **Editing out of context.** Before 2018.3 you edited a prefab by dragging a temporary copy into the scene.

The fixes all share one pattern: a separate, clearly marked editing context (Unity's Prefab Mode and stages, Unity ECS data modes), overrides you can see, and per-property apply. Beginners' complaints about Unreal are mostly about clutter and vocabulary. On first contact Godot is perceived as simpler than Unity, but it has less help available as tasks get harder.

### Cited Findings
**Unity prefab workflow redesign (2018.3)**
- The old workflow, as Unity describes it: to edit a Prefab Asset you dragged it into an open scene, modified it, applied the changes, and had to remember to delete the temporary copy. The Inspector's Apply button could push changes to the asset unintentionally, with no good way to see what would be affected. Mads Nyholm, June 20, 2018. — [Unity blog: Introducing new Prefab workflows](https://unity.com/en/blog/technology/introducing-new-prefab-workflows)
- The research basis was conversations with "a range of indie and AAA studios" about how they use prefabs day to day. The post describes no formal study. — [same](https://unity.com/en/blog/technology/introducing-new-prefab-workflows)
- What changed:
  - **Prefab Mode** edits the asset in isolation. Structural changes no longer prompt you to break the prefab connection.
  - **A separate Prefab Stage.** The main stage stays loaded, so the scene remains visible.
  - **Instances can no longer be disconnected to change their structure.** You either edit in Prefab Mode or "unpack" the instance.
  - **Better override visualization**, and overrides can be applied per property, per component or GameObject, or for the whole instance. — [same](https://unity.com/en/blog/technology/introducing-new-prefab-workflows)
- Unity's 2018.3 release post says the team ran interviews, usability tests and research at game jams, which showed users needed more than nesting alone. It shipped nested prefabs, Prefab Mode and variants. A dedicated UX designer for Prefabs presented the work at Unite, and a GDC 2019 talk covered the UI/UX reasoning. — [Introducing Unity 2018.3](https://unity.com/blog/technology/introducing-unity-2018-3); [Unity prefabs page](https://activation.unity3d.com/prefabs)
- The redesign was not uniformly welcomed. A forum thread titled "New Prefab workflows got worse (Unity 2018.3)" reports new restrictions and workarounds. **[anecdotal]** — [Unity Discussions](https://discussions.unity.com/t/new-prefab-workflows-got-worse-unity-2018-3/725295)

**Unity play mode**
- Unity's documentation says changes made in the Editor during Play Mode reset when you exit Play Mode (Unity 6.x manual). — [Unity Manual: Configurable Enter Play Mode](https://docs.unity3d.com/Manual/ConfigurableEnterPlayMode.html)
- Enter Play Mode Options let users skip the domain reload and/or the scene reload to start faster. With domain reload off, static state carries over between runs, so this trades speed for correctness. Unity says that by default in newer versions it does not reload the scripting domain on entering Play mode. A script change still triggers a domain reload during an asset refresh. — [Unity Manual: Domain reloading (6000.x)](https://docs.unity3d.com/Manual/domain-reloading.html)
- A bug report filed in March 2025 against Unity 6: the UI Builder discards unsaved UXML changes on entering Play Mode when both domain reload and scene reload are enabled. This is an example of losing work at the edit/play boundary. — [Unity Issue Tracker](https://issuetracker.unity3d.com/issues/ui-builder-unloads-uxml-file-and-discards-any-unsaved-changes-when-entering-play-mode-with-domain-and-scene-reload-enabled)

**Unreal**
- **[anecdotal]** A September 2023 forum thread, "Improving the first-user-experience of the editor", makes these complaints:
  - The UE5 Content Browser is hidden as a drawer the first time you open the editor, and any click hides it again.
  - A new project's Outliner shows rarely used items such as the Datalayer actor and the HLOD folder.
  - Marquee selection keeps selecting the Landscape.
  - Content Browser naming rules are confusing, and delete is buried in the Edit menu.
  - "Actor" is unintuitive vocabulary for beginners.
  - One poster argues Epic is losing new developers to Godot because of this first-user experience. — [Unreal Engine forums](https://forums.unrealengine.com/t/improving-the-first-user-experience-of-the-editor-to-attract-new-developers/1307824)

**Godot**
- **Feature profiles (Godot 3.2).** Users can hide the 3D editor, the asset library, docks, node types, resources, individual properties and contextual editors. The motivations given:
  - Teachers asked for it so a course could unlock features gradually.
  - Companies wanted to restrict non-programmers to the tools their jobs need.
  - Solo 2D developers wanted the 3D clutter gone.
  
  Juan Linietsky, April 8, 2019. — [Godot blog](https://godotengine.org/article/godot-32-will-allow-disabling-editor-features)
- Godot's docs support saving and restoring named editor layouts, giving the example that animation work and level design want different dock arrangements (4.x docs). — [Godot docs: Customizing the interface](https://docs.godotengine.org/de/4.x/tutorials/editor/customizing_editor.html)
- Godot 4.0 reworked how sub-resources appear in the Inspector. The developers admitted they had "never really managed to nail how to deal with sub-resources". — [Godot blog: Editor improvements for Godot 4.0](https://godotengine.org/article/editor-improvements-godot-40)
- **[anecdotal]** Users say the dock system's fixed positions are limiting, for example there is no bottom dock position for the side docks. — [Godot forum](https://forum.godotengine.org/t/why-doesnt-godot-let-you-dock-anything-at-the-bottom-of-the-screen-and-you-cant-undock-console-debugger-etc-either/69453)

**Small comparative studies (low N, weak evidence)**
- An Uppsala student cognitive task analysis with 4 participants found Godot's simpler GUI easier at first. As tasks got harder, students needed the teacher. Unity starts harder but has more solutions online. — [uppsatser.se](https://www.uppsatser.se/uppsats/8a866e4082/)
- An SBGames single-participant case study found Godot judged the most intuitive engine, while Unity was viable but had a steep learning curve. — [SBGames](https://sol.sbc.org.br/index.php/sbgames/article/download/45568/45329/)

### Inferences
- Unity's lessons transfer directly to our design. Editing a reusable definition (a prefab) belongs in a visibly distinct context. "Apply" must show what it will touch, and overrides should be visible per field. Flecs prefabs with `OnInstantiate Inherit`, which we already use for UI styles, raise the same question of override visibility.
- The "play mode changes are lost" problem comes from one invisible boundary between authored state and runtime state. Unity's later ECS answer was to label runtime-only data in color and let the user choose the data mode (see section 3).
- Feature profiles are a vendor-validated way to get a low floor without lowering the ceiling: the same editor, with parts hidden.

### Gaps
- No primary Epic UX-team blog post on the Details panel or Content Browser design was found within the search budget.
- No Godot UX-team "first hour" study was found. The godot-proposals repository has many usability proposals, but none was summarized here.
- The GDC 2019 Unity prefab UX talk content was not retrieved.

## 2. Beginner-oriented editors: what makes the first hour smooth

### Takeaway
The best-documented beginner tools succeed through **constraint plus completeness**: everything you need is built in, nothing else is present, and the limits are deliberate. Examples are PICO-8, with fixed specifications and sprite, map, sound and code editors in one place, and Bitsy, with 8×8 tiles, 16×16 rooms and two colors per palette. A peer-reviewed study gives the strongest evidence about help: **interactive in-editor help beats text docs**, and having no help is clearly worst.

### Cited Findings
- **Kao, "Exploring Help Facilities in Game-Making Software"** (FDG 2020, N=1646 on Mechanical Turk, between-subjects). There were six conditions: text, interactive tutorial, intelligent agent, video, all, and none. Each participant built a first-person-shooter level.
  - Interactive Help had the largest positive effect on time spent, controls learnability, learning motivation, cognitive load, total editor activity and level quality. Video Help came a close second.
  - No help was worst on every measure.
  - Text, the intelligent agent and "all" did not differ significantly from no help, except on cognitive load.
  - In a review of 85 game-making tools, most had text help, about half had video, and few had interactive help. — [arXiv 2006.03519](https://arxiv.org/abs/2006.03519)
- **PICO-8.** It opens to a shell with built-in tools for code (Lua), sprites, maps and sound. Its limits were "carefully chosen to be fun to work with" and to encourage compact designs. — [Gentoo package description quoting Lexaloffle](https://packages.gentoo.org/packages/dev-lang/pico8); [Raspberry Pi Magazine review](https://magazine.raspberrypi.com/articles/pico-8-review). Commentators credit the fixed specifications with helping creators actually finish games. — [Atomic Object](https://spin.atomicobject.com/pico-8-review/) (secondary)
- **Bitsy** (Adam Le Doux). He built it for himself to stay focused on walking, talking and exploring. The constraints are intentional (8×8 tiles, 16×16 rooms, two colors per palette). The community has been content with it remaining simple. Wired reports that the first interface was designed around his wife as its main user. — [Wired](https://wirednews.pages.dev/story/you-make-fun-games-no-experience); [Quinnipiac paper on tiny engines](https://mywebspace.quinnipiac.edu/jbwarren/archive/2022-175/images/inception/WarrenJB-Tiny-Online-Game-Engines.pdf) (secondary)
- **GDevelop.**
  - Logic is an event sheet of conditions and actions, not "a wall of code". Events can be edited, deleted and undone in the sheet. — [GDevelop wiki: events editor](https://wiki.gdevelop.io/gdevelop5/interface/events-editor/)
  - Ready-made behaviors give game-specific vocabulary (platformer, top-down movement and so on). Users can write their own behaviors as events, with their own conditions, actions and expressions. The docs recommend moving logic into behaviors as a project grows, so the scene's event sheet does not get cluttered. — [GDevelop wiki: events-based behaviors](https://wiki.gdevelop.io/pt/gdevelop5/behaviors/events-based-behaviors); [GDevelop basic concepts](https://wiki.gdevelop.io/gdevelop5/tutorials/basic-game-making-concepts/)
- **Dreams (Media Molecule, 2020).** Onboarding uses dozens of tutorials, plus guide characters inside the game (the Dream Queen, the Architect, the Designer). A single cursor, the "imp", is the one tool for browsing, sculpting and logic. Users are steered to start simple with "Art's Dream", a game made with the same tools. Remixing and borrowing other people's creations is first-class, and there are weekly community jams. — [PlayStation: Dreams beginner's guide](https://www.playstation.com/en-us/editorial/dreams-beginners-guide)
- **Super Mario Maker (2015).** According to director Yosuke Oshino, the idea began as a course editor for the side-scrolling Mario games. The interview asks about designing an interface anyone can pick up, but his answer was not retrieved. — [Nintendo Life interview](https://www.nintendolife.com/news/2015/09/interview_yosuke_oshino_on_the_origins_and_evolution_of_super_mario_maker)

### Inferences
- Help built into the editor, with a step-through tutorial that drives the real UI, is the cheapest proven win for the first hour. Text docs alone measured no better than nothing on most outcomes in the Kao study.
- Beginner tools expose game vocabulary, such as "platformer behavior", "room" and "dialog", rather than engine vocabulary such as "Actor", "Node" or "entity". Unreal's "Actor" complaint (section 1) is the negative example.
- Remix and borrow (Dreams), and finishing within a box (PICO-8, Bitsy), are motivational design rather than UI chrome. Templates and example projects you can open and change belong in the first-hour flow.

### Gaps
- No primary sources were retrieved for GameMaker, RPG Maker, Construct, Roblox Studio or LittleBigPlanet within the budget.
- No Nintendo primary source explains Mario Maker's UI choices, such as touch-screen placement, the sound-effect stamps and instant play-test toggling.
- The Dreams GDC or Siggraph talks were not retrieved.

## 3. Data-oriented / ECS editors: presenting entities and systems that are not a tree

### Takeaway
ECS editors take one of three approaches:
1. **Hide the ECS behind an authoring layer** and convert it (Unity DOTS "baking").
2. **Expose the live world as a queryable database** (Flecs Explorer).
3. **Wait for foundations** (Bevy, which has been blocked for years on its UI and scene format).

The Unity Entities editor's key UX device is a **data-mode switch** (Authoring, Runtime or Mixed), with runtime-only values tinted so users know which edits will survive.

### Cited Findings
- **Unity Entities (1.x through 6.x packages).**
  - The Entities Hierarchy and Inspector share a data mode selector (a circle at the top right of the window) with the modes Automatic, Authoring, Mixed and Runtime.
  - Authoring mode lists GameObjects and prefab instances. Runtime mode shows entities that exist only at runtime. Automatic mode uses Authoring in Edit mode and Mixed in Play mode.
  - Edits made in Authoring mode update the authoring value and persist after Play mode ends.
  - Runtime data that will be destroyed on exiting Play mode is highlighted: orange in the dark theme, red in the light theme. — [Unity Entities: Working with authoring and runtime data](https://docs.unity3d.com/Packages/com.unity.entities@1.0/manual/editor-authoring-runtime.html); [Entities Hierarchy window (6.5)](https://docs.unity3d.com/Packages/com.unity.entities@6.5/manual/editor-hierarchy-window.html); [Entity Inspector (6.4)](https://docs.unity3d.com/Packages/com.unity.entities@6.4/manual/editor-entity-inspector.html)
  - In the 0.50 and 0.51 packages the "DOTS Hierarchy" was read-only and needed Live Conversion to show the full hierarchy in Edit mode. The read/write split took several releases to settle. — [DOTS Hierarchy 0.51](https://docs.unity3d.com/Packages/com.unity.entities@0.51/manual/editor-hierarchy-window.html)
  - Unity also ships an Entities **Query window** for inspecting queries. — [Query window reference 1.3](https://docs.unity3d.com/Packages/com.unity.entities@1.3/manual/editor-query-window.html)
- **Flecs Explorer.**
  - It is a web UI for "monitoring Flecs applications, trying out queries & learning ECS". It runs live at flecs.dev/explorer. — [GitHub flecs-hub/explorer](https://github.com/flecs-hub/explorer)
  - The entity inspector can show incoming relationship edges, and an alerts addon has an explorer UI that watches queries for invalid patterns. — [Flecs releases](https://github.com/SanderMertens/flecs/releases?page=2)
- **Bevy editor.**
  - Bevy's fifth-birthday post (Aug 23, 2025) says the editor was "blocked by features bevy didn't have yet": BSN scenes and better UI such as headless widgets. It is volunteer work with long design iteration.
  - Principles borrowed from Blender ("Respect the user"): **Non-Overlapping, Non-Blocking, Non-Modal**.
  - Core tenet: "Make the hard parts of Bevy easy."
  - Explicitly *not* an asset-creation tool, an IDE replacement, or **a tool for editing live game state**. Its primary job is creating and editing scene (BSN) files.
  - Tabbed, tiling layout like Blender. Plans include visualizers for systems, observers and events (a system-ordering view, possibly a node graph), and a launcher for projects and templates. — [Rin Oxide, "Bevy's Fifth Birthday: The Editor"](https://rinoxide.substack.com/p/bevys-fifth-birthday-the-editor)
  - The Feathers widget set for tools shipped around Bevy 0.17–0.18. Bevy 0.19 (June 2026) added more editor widgets and ported them to BSN. The official prototypes repository still calls itself a work in progress. — [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/); [bevy_editor_prototypes](https://bevyengine.github.io/bevy_editor_prototypes/print.html)
- **Our Machinery / The Machinery** (Niklas Gray, Feb 18, 2018), on undo models:
  - **Per-document undo** (like Visual Studio) lets you undo in one file without touching others. But a multi-asset operation such as find-and-replace across 200 files must be undone file by file. Renames don't fit. Undoing in one asset can break another, for example an animation left referring to an undone bone.
  - **A single project-wide stack** avoids those problems but can surprise users by undoing changes in documents they have closed.
  - Collaborative editing needs one undo stack per user.
  - UX stance: "take a clear stance rather than offering many configuration options", because options fragment users and tutorials. All tabs and windows are equals. Relations such as a preview following the asset browser are resolved at render time rather than stored. — [Our Machinery blog archive: The Document Model and The Machinery](https://ruby0x1.github.io/machinery_blog_archive/post/the-document-model-and-the-machinery)
  - The companion post on "The Truth" says a data model that records a history of changes makes undo a matter of rewinding that history, which is simpler than the command pattern. — [Game Developer: The Story behind The Truth](https://gamedeveloper.com/programming/the-story-behind-the-truth-designing-a-data-model)

### Inferences
- Our design already has a "Flecs-explorer-based inspector". Unity's data-mode tinting is the missing UX piece: always show whether a value is authored (saved) or runtime (it will vanish on stop or restore).
- Systems are the hard thing to show. No surveyed editor had a mature view of systems. Bevy plans one, Flecs Explorer offers query views, and Unity has a Query window. Our "systems are queries" model makes a query-result table a natural view: entities as rows, components as columns.
- Bevy's choice not to edit live game state is the opposite of our play-in-editor-by-snapshot plan. That is fine, but it shows the boundary must be explicit.
- Our transaction-log undo should decide the per-document versus global question up front. Gray lays out the failure modes of each.

### Gaps
- Defold's editor model (collections, game objects, components, and hot reload to a running device) was not researched in this pass.
- The Flecs Explorer's system and pipeline views were not documented in the retrieved sources.

## 4. Live editing and iteration time: hot reload, edit-while-playing, time travel

### Takeaway
Tomorrow Corporation's 2023 tech demo is the strongest public example of time travel in a game editor. It is **state-snapshot based** (heap copies at frame boundaries), not input-replay based, and it works because the games' state is small. Unity shows the cost of slow iteration: domain reload became optional and skippable at the cost of stale static state.

### Cited Findings
- **Tomorrow Corporation, "How We Make Games at Tomorrow Corp – Our Custom Tools Tech Demo"** (Allan Blomquist, Feb 21, 2023). The details are in his comment replies:
  - Coarse snapshots are taken every 2 minutes. Fine snapshots are created on demand around the current point, spaced exponentially further apart going back.
  - Snapshots happen only at frame boundaries and are essentially copies of the heap; the stack is not needed.
  - The first reverse step pauses briefly while seeking from a coarse snapshot. Later steps are fast.
  - Moving through time is not itself recorded, only "the evolution of the game's state".
  - Forking a new timeline from a past point is possible but not implemented.
  - It is not general purpose: it suits games with modest state, such as classic consoles with about 24 MB of RAM.
  - The custom language is transpiled to C to ship. — [Tomorrow Corporation blog](https://tomorrowcorporation.com/?p=3022)
- Community reaction praised the coding, debugging and refresh workflow and wondered about RAM and CPU cost. **[anecdotal]** — [Defold forum](https://forum.defold.com/t/tomorrow-corporation-tech-demo/76765); [Future of Coding](https://linen.futureofcoding.org/t/18768900/the-tooling-here-is-next-level-bret-victor-would-be-proud-ht)
- **Unity iteration.** Configurable Enter Play Mode exists to cut play-mode entry time by skipping the domain and scene reloads. A script change still forces a domain reload. — [Unity Manual: Domain reloading](https://docs.unity3d.com/Manual/domain-reloading.html)

### Inferences
- Our deterministic core (`Simulation::step`, `state_hash`, input recording) supports *both* of Tomorrow Corp's mechanisms: snapshots for seeking, and input replay between snapshots. Their exponential snapshot spacing is a concrete, proven scheme for scrubbing.
- Their "the timeline is not part of the recording" choice matches our "a tick is not a frame" separation. Scrubbing is a view, not simulation input.
- Forking from the past is the obvious next feature after scrubbing (edit at tick N, then resimulate). Tomorrow Corp left it unbuilt.

### Gaps
- The hot-code-reload and live-edit details of the Tomorrow Corp demo are in the video, not the post text, and were not transcribed.
- Braid's rewind implementation (Jonathan Blow's GDC talks) and Bret Victor's "Inventing on Principle" were not retrieved.
- No measured iteration-time studies (for example, seconds from edit to see-the-change in each engine) were found.

## 5. Common complaints: clutter, panels, modal settings, slow startup, losing work, layout chaos, search

### Takeaway
The recurring complaints in the sources are:
- **Clutter in default views**: Unreal's Outliner, Godot's 3D tools in 2D projects.
- **Hidden or collapsing panels**: the UE5 Content Browser drawer.
- **Edits lost at the edit/play boundary**: Unity Play Mode, the UI Builder bug.
- **Unintended bulk edits**: Unity's old prefab Apply.
- **Rigid docking**: Godot.

The vendors responded with hideable features (Godot feature profiles), saved layouts (Godot), non-modal and non-overlapping principles (Bevy and Blender), and a firm default rather than settings (Our Machinery).

### Cited Findings
- Clutter: rarely used default Outliner entries in Unreal **[anecdotal]** — [Unreal forums](https://forums.unrealengine.com/t/improving-the-first-user-experience-of-the-editor-to-attract-new-developers/1307824). 3D editor clutter for 2D users — [Godot blog](https://godotengine.org/article/godot-32-will-allow-disabling-editor-features).
- Panels that collapse on any click (the UE5 Content Browser drawer) **[anecdotal]** — [Unreal forums](https://forums.unrealengine.com/t/improving-the-first-user-experience-of-the-editor-to-attract-new-developers/1307824)
- Losing work at the edit/play boundary — [Unity Manual](https://docs.unity3d.com/Manual/ConfigurableEnterPlayMode.html); [Unity Issue Tracker](https://issuetracker.unity3d.com/issues/ui-builder-unloads-uxml-file-and-discards-any-unsaved-changes-when-entering-play-mode-with-domain-and-scene-reload-enabled)
- Accidental bulk changes with no preview of their reach — [Unity blog 2018](https://unity.com/en/blog/technology/introducing-new-prefab-workflows)
- Layout rigidity **[anecdotal]** — [Godot forum](https://forum.godotengine.org/t/why-doesnt-godot-let-you-dock-anything-at-the-bottom-of-the-screen-and-you-cant-undock-console-debugger-etc-either/69453); vendor mitigation by saved layouts — [Godot docs](https://docs.godotengine.org/de/4.x/tutorials/editor/customizing_editor.html)
- The design principle against modality and overlap — [Bevy editor post](https://rinoxide.substack.com/p/bevys-fifth-birthday-the-editor). Against configurability — [Our Machinery](https://ruby0x1.github.io/machinery_blog_archive/post/the-document-model-and-the-machinery)
- **[anecdotal]** The Godot inspector's numeric and text entry fields are "picky" to click into. — via search snippet, Godot forums (undated)

### Inferences
- Tear-off tabs (our Godot-like plan) bring a known risk of layout chaos. Named, resettable layouts per task, with a one-click "reset to default", are the vendor-tested mitigation.
- Our Machinery's "take a clear stance" argues for few preferences and a single good default, which also keeps tutorials valid. That matters at the low floor.

### Gaps
- No sourced evidence on search quality (fuzzy command palettes, asset search) in any engine was gathered.
- No quantitative data on startup or import times was found.

## Implications for our engine at each skill level

These implications are inferences from the cited findings above, mapped onto decisions already in the design doc (Scratch-style look, Mario-Maker scene editor, Flecs-explorer inspector, transaction-log undo, snapshot play-in-editor, live reload).

**Beginner (first hour)**
- Put **interactive tutorials that drive the real editor** first in onboarding. In the Kao FDG 2020 study (N=1646), interactive help was best and text alone was no better than none on most measures. Video is the second-best option.
- **Constrain and complete**, as PICO-8 and Bitsy do. Ship a "beginner profile" in the style of Godot feature profiles: hide the system and query views, the raw component columns and the Python tab until they are asked for. The same editor unlocks progressively. This keeps the low floor without forking the tool.
- Use game vocabulary on the Mario-Maker top bar ("Player", "Coin", "Wall", "moves like a platformer") instead of "entity" and "component". GDevelop's ready-made behaviors and Unreal's "Actor" complaint are the positive and negative examples.
- Make the **edit/play boundary unmissable**. Tint runtime-only values the way Unity Entities does (orange in dark, red in light). Never silently discard an edit made during play: offer "keep this change" per value, since we restore by snapshot anyway.
- Make example projects remixable from the start screen (Dreams' remixing, Mario Maker's course sharing).

**Intermediate**
- **Prefab and variant editing in a distinct context** with visible per-field overrides and a preview of what "apply" will change. This is Unity's 2018.3 lesson, and it applies directly to Flecs `IsA` prefabs and `OnInstantiate Inherit` styles.
- Behaviors as reusable bundles authored in blocks or Python, so scene logic does not sprawl. This follows GDevelop's events-based behaviors.
- Named, resettable layouts per task (Godot) to tame tear-off tabs, following non-overlapping and non-modal defaults (Bevy and Blender).
- Settle undo scope now. Per-document undo breaks multi-asset edits and renames; a global stack surprises users (Our Machinery). A transaction log with a per-transaction scope label, for example "renamed X in 3 files", can show the user what an undo will touch.

**Expert**
- **Time-travel scrubbing** using Tomorrow Corp's scheme: coarse snapshots plus exponentially spaced fine snapshots near the cursor. Our deterministic input replay fills the gaps between snapshots, and `state_hash` verifies them. Add **fork-from-tick** (edit, then resimulate from tick N), which Tomorrow Corp did not ship.
- **Query and system views as tables.** Systems are queries, so show "what this system matches" as a live row/column grid, plus a system-ordering view. Bevy plans this, and no shipped editor surveyed does it well, which is an opportunity.
- Keep iteration time measurable. Unity made reloads optional and paid for it in correctness bugs. Our live reload should keep determinism, and the `state_hash` check should say when a reload changed the simulation.
- Show relationships in the inspector (incoming `ChildOf` and other relationship edges, as the Flecs Explorer does), not only the tree.
