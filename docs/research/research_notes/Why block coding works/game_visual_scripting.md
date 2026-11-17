# Block-based and visual scripting systems for making games: what made each succeed or struggle

Scope note: research done 2026-11-14 with about 30 search and fetch calls. Coverage is uneven. Godot VisualScript, MakeCode, Unreal Blueprints, Unity Visual Scripting, GDevelop/Construct event sheets, Scratch's scheduler, Kodu, Alice, Stencyl, Dreams, PICO-8 and Droplet/Pencil Code have cited material. I found no usable sources for Hopscotch, Tynker, Bitsy, Flickgame, Super Mario Maker, or Roblox's official position on blocks. Those are listed under Gaps rather than filled from memory. Forum material is marked **(anecdotal)**.

## Which programming models suit games, and how do they map onto an ECS where systems are queries?

### Takeaway
Game tools have settled on three models: event hats with cooperative threads (Scratch, MakeCode, Stencyl), rule tables evaluated every frame (Kodu WHEN/DO, GDevelop and Construct event sheets), and dataflow/exec node graphs (Blueprints, Unity Visual Scripting, Dreams). The rule-table model is already an ECS system in disguise. Each event-sheet row is "filter the instances with conditions, then run actions on the survivors", every frame, top to bottom. That is a query plus a body, run in a fixed order.

### Cited Findings
**Rule tables (event sheets, Kodu)**
- GDevelop: "Events run in the order they are listed from top to bottom." With no conditions, actions run every frame, usually about 60 times per second. Conditions decide both *when* actions happen and *which objects* they affect ("filter" or "pick"); by default actions apply to all instances in the scene. Sub-events run only after the parent's conditions pass. Events can define custom conditions, actions and expressions (functions). — [GDevelop wiki: Events](https://wiki.gdevelop.io/gdevelop5/events/)
- Construct: each event starts with all instances picked, and each condition filters that set (the "SOL", selected object list, one per object type). Conditions are AND by default. If an event's conditions don't mention an object, its actions apply to every instance of that object; the manual's example is a spacebar event that destroys every Monster. Sub-events inherit the parent's picked set, and a new top-level event starts afresh. — [Construct manual: How events work](https://www.construct.net/en/animation-software/manual/project-primitives/events/how-events-work), [Construct forum: instances picked when no condition references the object](https://www.construct.net/en/forum/construct-3/how-do-i-8/instances-picked-event-doesnt-180316)
- Picking is a concept that "doesn't exist in JavaScript". Construct's script blocks inside event sheets can read the picked instances, but scripts have no picking of their own. — [Construct forum: passing the SOL to JS](https://construct.net/en/forum/construct-3/scripting-51/passing-sol-conditions-179337)
- Picking is the classic event-sheet pitfall: sub-event conditions re-filter the parent's set and can empty it, and family instance variables return an arbitrary member unless a condition or For Each narrows the set first. — [bugnet.io: sub-event scope losing picked instances](https://bugnet.io/blog/how-to-fix-construct3-sub-event-scope-losing-picked-instances), [bugnet.io: picking selects all instances](https://bugnet.io/blog/how-to-fix-construct3-picking-selects-all-instances-instead-of-one) (third-party troubleshooting articles)
- Construct's own JavaScript tutorial warns that event blocks "don't map neatly" onto JavaScript, so people moving over have to change how they approach problems. It presents mixing the two as the easier transition. — [Construct: Learn JavaScript, part 13](https://www.construct.net/en/tutorials/learn-javascript-construct-2835/page-3)
- Kodu (MacLaurin, "The design of Kodu: a tiny visual programming language for children on the Xbox 360", POPL 2011, DOI 10.1145/1926385.1926413). Programs are made of pages, pages of rules, and each rule splits into a condition and actions. — [researchr entry](https://researchr.org/publication/MacLaurin11), [Planet Kodu course](https://www.scribd.com/document/449078519/Planet-Kodu-Course-pdf). Press example: "When the motorcycle Sees the Apple it moves toward it. When it Bumps into the Apple it Eats it." — [Today.com](https://www.today.com/id/31755421/ns/technology_and_science-games/). MacLaurin: "We decided very early that we weren't trying to teach people Java or C#." It is programmed with an Xbox controller only. — [MCV](https://mcvuk.com/business-news/consoles/key-release-kodu/)

**Event hats with threads (Scratch lineage)**
- MakeCode programs are mostly event handlers (button A, button B, shake). The API is layered: high-level event handlers, then polling APIs, then low-level APIs such as I2C. — [Ball et al., SPLASH-E 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- Stencyl's Design Mode uses Scratch-derived blocks. An "Event" block detects when conditions are met or says when logic runs, and action blocks go inside it. Logic is packaged as reusable *behaviors* attached to actors and scenes. — [Wikipedia: Stencyl](https://en.wikipedia.org/wiki/Stencyl), [Stencyl glossary](https://www.stencyl.com/help/viewPrint/112)

**Draw loop (Processing lineage)**
- Code.org Game Lab: code outside `draw` runs once, and `draw` runs about 30 times per second, called by the environment ("a new page in a flipbook"). One draw loop per program. — [Code.org docs: The Draw Loop](https://docs.code.org/csd/draw_loop/), [CSD Lesson 7](https://curriculum.code.org/csd-19/unit3/7). A common student bug: resetting an animation every frame inside `draw`, so the sprite sticks on frame 1. The fix is to set it once when the key is first pressed. — [Code.org forum](https://forum.code.org/t/game-lab-unit-3-lesson-14/26148) (anecdotal)

**Dataflow / wire graphs**
- Dreams: logic is "gadgets" (sensors, timers, AND/OR/XOR/NOT gates) wired to objects. Microchips group gadgets to keep them tidy, and wireless transmitters and receivers send signals without wires. Reviewers say it's simplified "but it's still not easy". — [Dreams docs: Logic and processing gadgets](https://docs.indreams.me/en/create/resources/edit-mode-guide/assembly/gadgets/logic-and-processing), [Dreams docs: Sensors and input](https://docs.indreams.me/en/create/resources/edit-mode-guide/assembly/gadgets/sensors-and-input), [Push Square review](https://pushsquare.com/reviews/ps4/dreams)

### Inferences
- An event sheet row maps almost one-to-one onto a Flecs system: *object conditions* are query terms (`with`/`without`, plus value filters), *non-object conditions* ("key pressed", "timer elapsed") are guards on singletons, *actions* are the body over the matched entities, and *sheet order* is system order within a pipeline phase. The Construct rule "an object not named in the conditions means every instance" is exactly an unfiltered query. That makes rule tables the most natural block "shape" for systems. Our engine already has systems as queries, so a "When [query] / Do [actions]" block that maps to a `@system` is a direct fit.
- Picking is the main thing event-sheet users trip on, and it is implicit. A block view of a query should *show* the matched set (live count and highlighted entities in the editor) rather than leave it implicit.
- Kodu's WHEN/DO with pages as states maps onto systems that only run in a given `GameState` (Flecs module enable/disable, which S2 shows disables systems). A "page" is a group of systems enabled together.
- Event hats ("when this sprite clicked", "when I receive") map onto Flecs observers and events, not systems. A tool needs both: rule rows (per-tick systems) and hats (observers). Scratch users mix "forever" loops with hats; in an ECS the "forever" loop *is* the system.
- Wire graphs (Dreams, Blueprints) are dataflow-shaped. They suit signal logic (gates, timers) and materials or shaders more than per-entity game rules over many instances.

### Gaps
- I could not reach the Kodu paper's full text (the Microsoft Research PDF URL I tried returned 404), so the sensor/filter/actuator vocabulary, "rules evaluated every frame", and the rationale for leaving out variables and loops are **not verified** here. Read DOI 10.1145/1926385.1926413.
- The Construct manual page returned 403 to the fetcher. The picking facts above come from forum excerpts and search summaries of the manual.
- No source found for Super Mario Maker's (on/off switches, tracks, contraptions as no-code logic) or Roblox's design rationale.

## Why did Godot drop VisualScript, how is Unity Visual Scripting received, and what are the pain points of node graphs vs stacked blocks?

### Takeaway
Godot removed VisualScript in 4.0 because almost nobody used it (0.5% of more than 5,000 poll respondents used it as their main language). It was a node-graph transliteration of GDScript with no high-level game vocabulary, and beginners found GDScript easy enough anyway. Node graphs pay off when they sit over a rich, game-specific API (Unreal, which ships with packaged game features). Their known costs are speed (Unity's reflection runtime), binary assets that won't merge, and spaghetti on per-frame logic.

### Cited Findings
**Godot VisualScript (removed in 4.0; announced 23 August 2022)**
- "Despite our continuous effort, it never gained traction and the path to improve it was never clear." Many potential users "found GDScript easy to learn" even without programming experience. "Godot lacked high level components to make use of it". VisualScript examples were never added to the docs "due to technical reasons". The team "struggled to get enough feedback", and "the approach we took to visual scripting was simply not the right one." — [Godot blog: Godot 4 will discontinue visual scripting](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting)
- Poll: more than 5,000 respondents, and only 0.5% use VisualScript as their main engine language. In an earlier poll, most people who wanted visual scripting named the Blueprint style. VisualScript arrived in Godot 3.0 and stays supported in 3.x. The code moved to a separate repository and could become a GDExtension if volunteers take it on. Contributors floated GameMaker- or GDevelop-style alternatives as extensions. Visual *shaders* "work well" and stay. — [same](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting)
- The post contrasts Godot with Unreal, GameMaker and Construct, where visual scripting is paired with high-level, packaged game features; Godot's general-purpose design made that pairing harder. — [same](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting), [GameFromScratch coverage](https://gamefromscratch.com/visualscript-removed-from-godot-4/)
- As of 2025–2026, forum users still ask for visual scripting in Godot, and third-party plugins (for example Orchestrator) fill the gap. — [Godot forum: should visual scripting come back](https://forum.godotengine.org/t/visual-scripting-should-come-back-in-godot/127760), [Godot forum, 4.6.2 thread](https://forum.godotengine.org/t/4-6-2-can-a-game-be-made-with-godot-engine-4-x-x-ide-editor-without-using-any-gdscript-source-code/137388) (anecdotal)

**Unity Visual Scripting (formerly Bolt)**
- Unity acquired Ludiq's Bolt. Visual Scripting became a built-in package in Unity 2021.1, and later changelogs are mostly bug and performance fixes. — [Unity Visual Scripting 1.7 changelog](https://docs.unity3d.com/Packages/com.unity.visualscripting@1.7/changelog/CHANGELOG.html), [1.9 changelog](https://docs.unity3d.com/Packages/com.unity.visualscripting@1.9/changelog/CHANGELOG.html)
- Ludiq's own docs say the Bolt 1 runtime uses reflection to execute graphs. Bolt 2 was going to generate C#. — [Ludiq: C# Script Generation](https://ludiq.notion.site/C-Script-Generation-c2c333b4cd2a4a5fb631cbd2802b09e6)
- **(Anecdotal)** Forum users report it is 5–10x slower than C#, with boxing from `<string, object>` variable dictionaries. They say Bolt 2 was cancelled after the acquisition, that there has been little progress past Bolt 1 (2020), and that a promised code-generation runtime (described as 10–12x faster) never shipped. — [Unity Discussions: Bolt coding capacity](https://discussions.unity.com/t/bolt-coding-capacity/813347.md), [Unity Discussions: Bolt perf](https://discussions.unity.com/t/bolt-perf/936802), [Unity Discussions: UVS vs Blueprints vs C# performance](https://discussions.unity.com/t/performance-troubles-unity-visual-scripting-compared-with-unreal-blueprints-and-direct-c/883750). I found no official Unity statement confirming or denying "abandoned".

**Unreal Blueprints**
- UE4 had "Blueprint Nativization" (converting Blueprints to C++ at package time). It was removed in UE5. **(Anecdotal)** users found it unreliable, especially with circular references and interfaces. — [Unreal forum: Why was Blueprint Nativization removed?](https://forums.unrealengine.com/t/why-was-blueprint-nativization-removed-no-code-preaching/232490/23), [uhiyama-lab notes](https://uhiyama-lab.com/en/notes/ue/blueprint-performance-optimization-nativize/)
- The common advice is to keep heavy per-frame logic (Tick) in C++ and use Blueprints for glue. — [Unreal forum: good practices for performant Blueprint](https://forums.unrealengine.com/t/general-good-practises-to-write-performant-blueprint-code/94676) (anecdotal)
- Blueprints are binary `.uasset` files: Git and Perforce see "two completely different blobs" with no automatic merge. Unreal ships a visual diff and merge tool that shows the versions side by side but doesn't resolve conflicts itself. The usual advice is exclusive file locking. Third-party tools (BPMerge, TheCodeDiff) exist to fill the gap. — [Epic blog: Diffing Unreal assets](https://www.unrealengine.com/blog/diffing-unreal-assets?lang=zh-CN), [Diversion: .uasset version control](https://www.diversion.dev/knowledge-center-articles/uasset-version-control) (vendor), [Unreal forum: Pain of binary assets](https://forums.unrealengine.com/t/pain-of-unreal-engine-binary-assets/24394), [BPMerge announcement](https://forums.unrealengine.com/t/open-source-bpmerge-deterministic-3-way-merge-for-unreal-engine-blueprints/2722265)

**Stencyl (blocks over Haxe)**
- A behavior is permanently either Design Mode (blocks) or Code Mode (Haxe): "you cannot switch between design and code mode". To mix them, you put code blocks inside a Design Mode behavior. A mode switch is a popular community request. — [Stencyl help: Code Mode](https://www.stencyl.com/help/view/code-mode)

### Inferences
- The Godot lesson cuts against a "visual view of a general-purpose language". What made visual tools work elsewhere was the *vocabulary*: high-level, game-specific blocks. Our palette generated from reflection will only be as good as the native and kit components it reflects. A reflected palette of low-level calls would reproduce VisualScript's failure. Curated, game-level components and kit functions matter more than block mechanics.
- Godot's "GDScript was easy enough" finding is a warning and an opportunity: blocks must teach the text (they are a *view* of Python), not compete with it.
- Stacked blocks that serialize to plain Python text avoid the Blueprint merge problem entirely, because the source of truth is diffable text. That is a real advantage over node-graph tools to keep and to advertise.
- Unity's reflection-interpreted graphs show the cost of interpreting a visual layer separately. Because our blocks *are* Python, they run at Python speed, with no second runtime to fall behind.
- Stencyl's one-way lock (a behavior is blocks or code forever) is the anti-pattern a lossless round trip exists to avoid.

### Gaps
- I found no rigorous measurement of "Blueprint spaghetti" (readability studies of node graphs against text). The evidence is community lore.
- I found no current (2025–2026) official roadmap for Unity Visual Scripting.
- Unreal 5.x's latest diff and merge improvements were not checked.

## How does MakeCode handle blocks to text round trips, and what are its limits?

### Takeaway
MakeCode treats text (Static TypeScript) as the source of truth. Blocks are a smaller subset of the language, generated from typed API declarations with comment annotations, and anything that cannot be shown as blocks is kept as an uneditable grey "JavaScript block". Python is a separate restricted subset converted to TypeScript. The round trip works when code starts in blocks. Hand-written text often comes back grey, and forum users hit size limits and conversion bugs.

### Cited Findings
- "MakeCode... provides a subset of TypeScript (referred to as Static TypeScript or STS...) for end-user programming, and an even smaller language subset available via Blockly. Automated translation between Blockly and STS allows the user to switch editing paradigms (though not every STS construct, such as classes, is editable in Blockly)." — [Ball et al., "Microsoft MakeCode: Embedded Programming for Education, in Blocks and TypeScript", SPLASH-E 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- Blocks are generated from API declarations: "MakeCode uses metadata in the comments associated with a device API... to make that API available via Blockly... the types of the API are used to automatically determine the kind of block". A `void` return gives a statement block, and anything else gives an expression block. A block can have fewer parameters than the function (optional parameters hidden, with defaults set in metadata). API namespaces become toolbox categories. Users "for the most part code without types", and the text editor is labelled "JavaScript" so students and teachers recognise it. — [same](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- The 2018 BLOCKS+ paper "TypeScript: From JavaScript to Blockly and Back" (Ball et al.) defines the mapping from TypeScript declaration files to a Blockly editor for a library. — [SPLASH 2018 BLOCKS+](https://2018.splashcon.org/details/blocks%2B-2018-papers/6/TypeScript-From-JavaScript-to-Blockly-and-Back)
- Blocks and Static Python are both converted to Static TypeScript before compilation. Static Python leaves out `with`, `assert`, classes, `super()`, `global`/`nonlocal`, `raise`, `try`, generators, imports, sets, comprehensions, `await`/`yield`, f-strings and `*args`, and scoping differs slightly from Python 3. — [MakeCode Languages](https://makecode.com/language)
- Grey blocks: when code can't be converted to blocks, a grey JavaScript block preserves it and converts back to the original text. Its code can't be edited in blocks, but the block can be moved, copied and deleted. Many blocks need "a very specific structure to their JavaScript" to convert, and the docs recommend writing in blocks first for a reliable round trip. — [MakeCode docs: JavaScript Block](https://makecode.microbit.org/blocks/javascript-blocks), [Blocks-to-JavaScript course: Starter Blocks](https://makecode.microbit.org/courses/blocks-to-javascript/starter-blocks)
- **(Anecdotal)** A large Arcade project hit "Can't decompile because the script is too large". A MakeCode developer said (around beta v0.11.11) that blocks then JavaScript then blocks was valid for programs under 1500 blocks. Users report extension code failing to decompile, "Oops, there is a problem converting your code" bugs (confirmed by a developer), leftover grey "null" blocks from imperfect translation, and constructs such as `switch` with no block form. — [MakeCode forum: script too large](https://forum.makecode.com/t/my-code-seems-to-big-to-be-decompile-in-block/28637), [MakeCode forum: JS won't translate back to blocks](https://forum.makecode.com/t/javascript-wont-translate-back-to-blocks/11786), [MakeCode forum: Gray blocks and null](https://forum.makecode.com/t/gray-blocks-and-null/151), [MakeCode forum: extension decompile error](https://forum.makecode.com/t/error-with-custom-extension-when-switching-from-javascript/79)
- MakeCode's docs render every block image on the fly from its text equivalent, so hundreds of screenshots never need maintaining. Tutorials run inside the editor. — [Ball et al. 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- Arcade has a built-in pixel editor for sprites, and users can work "from customizing sprites... all the way to building custom AI". — [same](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)

### Inferences
- MakeCode confirms our design's core bet: text is the source of truth, typed declarations generate the palette, and anything without a block form is kept, not lost. Our `libcst` approach can do better than grey blocks in two ways. Keep comments and formatting (libcst is concrete-syntax), and make the fallback "Python block" *editable as text inline* rather than read-only.
- MakeCode's "write it in blocks first" advice shows that decompilation is pattern matching on canonical shapes. Hand-written idiomatic code drifts from those shapes. We should test the round trip on *hand-written* Python (corpus tests from real scripts such as `pong.py`), not only on code generated from blocks.
- The 1500-block decompile limit and the "too large" failures suggest the round trip must be incremental and fast: per function or per system, not per file.
- Rendering docs and tutorial images from code is cheap and keeps documentation in step with the palette, which is worth copying.

### Gaps
- The 1500-block figure is old (about v0.11). Current MakeCode limits were not found.
- I found no published grey-block rate (the share of real user programs that won't decompile).
- How MakeCode converts STS back to Python (the Python view) is not documented on the languages page.

## What makes event sheets (GDevelop/Construct) popular with hobbyists, and is there evidence of retention or graduation to text?

### Takeaway
Event sheets are popular because each row reads like a sentence (condition, then action), they run per frame over a high-level engine vocabulary (behaviors, picking), and users never face a blank text file. Both GDevelop and Construct offer an escape hatch into JavaScript inside the sheet, positioned as "advanced". I found no retention or graduation statistics.

### Cited Findings
- GDevelop's JavaScript code events are "only meant to be used by advanced users who are familiar with JavaScript programming". Extensions are recommended for reuse. — [GDevelop wiki: JavaScript Code events](https://wiki.gdevelop.io/gdevelop5/events/js-code)
- GDevelop positions itself as the next step after Scratch: the event system "looks and feels like an advanced version of Scratch's logic blocks" but runs on JavaScript, and teachers can add JS snippets as students are ready. — [GDevelop blog: Level up your Scratch students](https://gdevelop.io/blog/level-up-scratch-students-with-gdevelop)
- **(Anecdotal)** GDevelop forum: most tutorials target beginners, and the recommended way to make games is events and behaviors; JS is for advanced users. — [GDevelop forum](https://forum.gdevelop.io/t/why-there-is-no-tutorial-about-expanding-the-engine-with-javascript/41421)
- Construct lets scripts sit inside event sheets as a bridge, and its tutorials teach JavaScript and TypeScript inside Construct. — [Construct manual: Scripts in event sheets](https://www.construct.net/en/animation-software/manual/scripting/using-scripting/scripts-in-event-sheets), [Construct: Learn TypeScript, part 14](https://www.construct.net/en/tutorials/learn-typescript-construct-3232/page-3)
- GameMaker renamed "Drag and Drop" to "GML Visual" (with "GML Code" as the text counterpart) in release 2022.3, to stress that both are the same language. The mode is set per project, and it is aimed at novices. — [YoYo Games: Release 2022.3](https://www.yoyogames.com/en/blog/release-2022-3), [Wikipedia: GameMaker](https://en.wikipedia.org/wiki/GameMaker)
- Godot's poll found most people who wanted visual scripting meant Blueprint style, yet few used Godot's version. Stated wants did not turn into use without high-level vocabulary. — [Godot blog](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting)

### Inferences
- The event-sheet appeal lies in its built-in vocabulary (behaviors such as "platformer character", "bullet") plus the sentence-shaped row, more than in any graphical novelty. For us, the equivalent is shipping prefab "behaviors" (components plus kit systems) that blocks can attach. That is the ECS idea of adding a component to gain behavior.
- GameMaker's rename shows the industry moving toward "visual is a view of the real language", which matches our design.
- Both event-sheet tools graduate users into an inline script cell, not a separate file. A "Python cell inside the block stack" is the analogous bridge, and our lossless view goes further.

### Gaps
- I found no published retention data, user surveys or graduation rates for GDevelop, Construct or GameMaker visual users. GDevelop's own analytics are not public in anything I found.
- Construct's design rationale for picking was not found in a primary source.

## Concurrency and timing models in block tools, and how they map onto a fixed-timestep deterministic simulation

### Takeaway
Scratch and MakeCode both use cooperative green threads that yield only at specific blocks (wait, loop ends, broadcast-and-wait). Scheduling is deterministic in principle but depends on order. Event sheets and Kodu instead re-run all rules each frame with no threads. For a fixed-timestep deterministic sim, rule-per-tick is trivially deterministic. Wait/yield scripts can be made deterministic if a script's resume point and timers are simulation state (hashed and snapshotted) and scripts resume in a stable order.

### Cited Findings
- Scratch yield points are wait and wait-until, the end of each loop iteration, glide, ask, and broadcast-and-wait, plus the end of a script. Scheduling is cooperative: a script keeps control until it yields. — [arXiv 2607.00563, Certificate-Carrying Transformation of Event-Driven Block Programs](https://arxiv.org/pdf/2607.00563), [arXiv 2607.00623, SchedCheck: Schedule-Robustness Analysis](https://arxiv.org/pdf/2607.00623), [Scratch forum](https://scratch.mit.edu/discuss/post/3680396/)
- Scratch runs one pass over all running scripts, then a screen refresh, unless turbo mode is on. `wait 0` pauses until the next frame. — [Scratch forum](https://scratch.mit.edu/discuss/post/2746172), [Scratch forum](https://scratch.mit.edu/discuss/post/3371380) (anecdotal). Recent research (SchedCheck, 2026) studies how *robust* Scratch programs are to schedule changes, which implies that real programs do depend on script order. — [arXiv 2607.00623](https://arxiv.org/pdf/2607.00623)
- Broadcast starts the receiving scripts, and broadcast-and-wait holds the sender until they finish. — [arXiv 2607.00623](https://arxiv.org/pdf/2607.00623)
- MakeCode: "implicit nonpreemptive multi-threading... As in Scratch, only specific functions (namely pause and wait-for-event) can yield to other threads, making it simpler to reason about integrity of data and to avoid races." In the browser, the compiler emits continuation-passing JavaScript ("big switch statements inside of a while loop", locals in explicit stack-frame objects) so any function can suspend. Debug builds add suspension points before every statement for stepping and breakpoints. The native runtime uses fibers. — [Ball et al. 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- GDevelop: events run top to bottom every frame, and frame rate varies by machine, so the docs recommend `TimeDelta()` for changes over time. — [GDevelop wiki: Events](https://wiki.gdevelop.io/gdevelop5/events/)
- Code.org Game Lab: `draw` runs at a fixed rate set by the environment (about 30 per second). — [Code.org docs](https://docs.code.org/csd/draw_loop/)

### Inferences
- In our sim, a tick is fixed (`TICK_DT`), so there is no `TimeDelta()` trap. "Every frame" in a block should mean "every tick", and blocks should count ticks or seconds of simulation time, never wall time.
- Scratch-style `wait N seconds` inside a script means a suspended coroutine. To stay deterministic and snapshot-safe, the suspended state must live in components (for example a `Wait{until_tick}` component, or a state-machine component), not in Python generator frames, which can't be hashed or saved. MakeCode's CPS transform (locals in explicit frame objects) is the precedent for turning a script with waits into explicit state. A block `wait` could compile, in the *Python view*, into an explicit state field plus a tick check. Or, a lighter option: limit waits to "timer" components that observers react to.
- Broadcast maps cleanly onto Flecs custom events and observers. Broadcast-and-wait has no clean ECS equivalent and is probably best left out or modelled as a state change.
- Scratch's script-order dependence is a known source of bugs. Our rule "Python systems run in StableId order" plus explicit system order already gives a defined order. The editor should *show* that order (the sheet order) the way event sheets do.

### Gaps
- I did not find Maloney et al. 2010 ("The Scratch Programming Language and Environment", ACM TOCE) in full text. The Scratch scheduler details above come from 2026 arXiv papers and forums.
- I found no source on how Snap! or Stencyl schedule "wait" relative to frames.

## Built-in asset tools and the first experience; minimal tools as comparison

### Takeaway
The successful beginner tools put art next to code (MakeCode Arcade's pixel editor, Scratch's paint editor and library, Stencyl's actor editor), and the minimal tools (PICO-8) bundle *everything* under hard limits, which users credit with helping them finish games. Evidence is mostly design statements and anecdote rather than studies. The strongest research result is that framing (storytelling) changes engagement a lot.

### Cited Findings
- MakeCode Arcade includes a built-in pixel editor for sprites, and its whole design centres on removing installation barriers (a web app that works offline once loaded, because school computers are locked down and networks are unreliable). — [Ball et al. 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf)
- Alice: drag-and-drop prevents syntax errors, and programmers watch commands unfold as animation, which makes errors visible. — [CACM](https://cacm.acm.org/?p=25154), [UBC MET wiki](https://wiki.ubc.ca/MET:Alice:_Computer_Programming_in_a_Constructionist_Environment) (descriptive, not measured)
- Storytelling Alice (Kelleher, Pausch, Kiesler 2007; middle-school girls): compared with generic Alice, a 42% increase in programming time, students three times as likely to keep working on their projects, and no reduction in programming concepts learned. — [eWeek](https://www.eweek.com/development/girls-ask-alice-for-programming-skills/), [Wikipedia: Alice](https://en.wikipedia.org/wiki/Alice_(software))
- PICO-8: cart code is capped at 8192 tokens. The limits were chosen to foster creativity, and **(anecdotal)** developers say the cap discourages feature creep and builds a shipping mindset, while others complain about cutting features and "golfing" tokens. TIC-80 is the less-limited alternative. — [PICO-8 wiki: Tokens](https://pico-8.fandom.com/wiki/Tokens), [Level Up: PICO-8 for people who want to finish a game](https://levelup.gitconnected.com/pico-8-the-fantasy-console-for-people-who-actually-want-to-finish-a-game-b5e3be2ce833), [itch devlog: limitations as productivity hack](https://itch.io/devlog/678418/using-limitations-as-a-productivity-hack.amp)
- Dual mode (Droplet / Pencil Code): in Bau et al.'s study, eight middle schoolers with no prior experience used blocks most at first and moved to text more each day. In Holbert and Weintrop's logs, the blocks most often added after switching from text back to blocks were movement (30.5%), art (21.6%) and control (18.0%). Weintrop, Bain and Wilensky compared text, blocks and hybrid sections (about 90 students). — [Holbert & Weintrop 2017](https://ccl.northwestern.edu/2017/HolbertWeintrop.pdf), [Weintrop et al., Blocking Progress](https://ccl.northwestern.edu/2017/BlockingProgress.pdf), [Raspberry Pi research seminar summary](https://www.raspberrypi.org/blog/block-based-programming-does-it-help-students-learn/)

### Inferences
- First-run success depends on art, sound and logic in one window with no install, plus a high-level vocabulary. Our M1 asset pipeline and editor should include at least a pixel editor and a starter sprite and sound library before a block editor is shown to beginners.
- The Droplet data supports *free switching*: novices start in blocks and drift to text, and go back to blocks to find APIs (movement, art). That is a strong argument for keeping the palette usable as API discovery even for text users.
- PICO-8's limits suggest small, scoped starter templates ("make Pong", "make a platformer") rather than an empty project.

### Gaps
- No sources found for Bitsy, Flickgame, Hopscotch, Tynker or Super Mario Maker. The search for "Bitsy minimal tool" returned nothing relevant. These systems are uncovered in these notes.
- Snap! was not researched beyond what the design doc already uses (first-class procedures and nested calls).
- No study isolates the effect of a built-in paint editor on retention.

## Implications for our engine

### Takeaway
Our design (lossless Python view, typed sockets, palette from reflection) sits on the side the evidence favours: text as the source of truth (MakeCode, GameMaker's rename), diffable source (against binary Blueprints), and one runtime (against Unity's interpreted graphs). The biggest risks are the ones that sank Godot VisualScript: a palette that is low-level and general-purpose rather than game-level, and too little documentation and examples. For an ECS, the right top-level block shape is the event-sheet rule (a query plus actions, in visible order), with Scratch-style hats for observers.

### Cited Findings
- See the sections above. Each point below rests on findings cited there: [Godot blog](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting), [Ball et al. 2019](https://www.microsoft.com/en-us/research/uploads/prod/2019/10/splashws19splashemain-p8-p-41a6cf2-42682-final.pdf), [GDevelop events](https://wiki.gdevelop.io/gdevelop5/events/), [MakeCode JavaScript block](https://makecode.microbit.org/blocks/javascript-blocks), [Stencyl Code Mode](https://www.stencyl.com/help/view/code-mode), [Diversion on .uasset](https://www.diversion.dev/knowledge-center-articles/uasset-version-control).

### Inferences
1. **A "When / Do" rule block maps to a system.** The WHEN part is the query (components the entity has or lacks, value filters, singleton guards such as input); the DO part is the body. The sheet order is the system order inside a pipeline phase and should be visible and draggable. That is the event-sheet and Kodu model, and it is native to "systems are queries".
2. **Hat blocks map to observers and events** ("when X begins touching Y" as `ContactBegin`, "when I receive" as a custom event). Keep these separate from per-tick rules, as Scratch does.
3. **Show the picked set.** Picking is the event-sheet pitfall. In the editor, a rule's WHEN block should show a live count and highlight the matched entities.
4. **Pages are game states.** Kodu-style pages map to groups of systems enabled together (flecs module enable/disable, per S2).
5. **Wait and timing must be state, not frames.** Blocks such as `wait N ticks/seconds` must keep their resume state in components (hashed, saved, rewound). Do not use Python generators. Consider having no free-floating `forever` or `wait` scripts at all, offering timers and state components instead, or define a CPS-style conversion like MakeCode's. "Every frame" means "every tick"; no `TimeDelta`.
6. **Vocabulary before mechanics.** Curate game-level components and kit functions (movement, spawn, score, collide, play sound, behaviors as prefabs) and annotate them for the palette, as MakeCode's comment metadata does: hidden optional parameters, block text, categories. A raw reflected palette risks repeating VisualScript.
7. **Round trip that beats grey blocks.** The fallback "Python block" should be editable inline, keep comments, and work per function or per system. Test the round trip on hand-written scripts, not only on code generated from blocks. Avoid Stencyl's one-way lock.
8. **Keep text diffable.** Blocks serialize to the Python file itself (no side files that hold logic), which avoids Blueprint-style merge pain. Layout metadata, if any, should be optional and in comments or a separate file that can merge.
9. **First experience.** No install where possible, a pixel and sound editor next to the blocks, starter templates, and tutorials whose block images render from code (MakeCode docs). Framing matters (Storytelling Alice's 42% increase in programming time).
10. **Wire graphs are for signals and shaders.** If we ever add a node-graph view, use it for materials and shaders (Godot kept visual shaders) or signal logic (Dreams), not as the main game-logic view.

### Gaps
- Read Kodu (POPL 2011) and Maloney 2010 in full to confirm the per-frame rule semantics and the yield model before relying on them for design-doc changes.
- I found no quantitative evidence comparing stacked blocks with node graphs for game logic.
