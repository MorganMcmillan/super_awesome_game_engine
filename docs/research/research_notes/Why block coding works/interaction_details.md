# Interaction details: the micro-level UX and runtime choices behind Scratch-like block editors

Scope: liveness, a runtime that fails soft, concurrency and frame timing, drag-and-drop ergonomics, discoverability, getting started, accessibility and localization, and scaling. The design doc already covers block shapes, typed sockets, suggestions narrowed by type, palettes from reflection, keyboard navigation of blocks, comments and text round trips, so those are left out.

Research date: 2026-11-14. Primary sources read in this session:
- scratch-vm `sequencer.js`, `runtime.js` and `cast.js` (develop branch).
- Blockly `core/config.ts` (develop branch).
- Fraser 2015, full text.
- Tanimoto 2013, full text.
- Victor, "Learnable Programming".
- The Blockly accessibility page.

The full text of Maloney et al. 2010 could not be retrieved: the MIT host refused the connection and the mirrors returned binary or empty pages. Claims attributed to it below come from search snippets or a one-page summary, and are flagged where that matters.

## 1. Liveness: click to run, glow, editing while running, always-visible stage, the green flag and stop sign

### Takeaway
Scratch's "feel" rests on two properties. Code is always "live": any fragment runs when clicked, and you can edit while it runs. Running code is visibly marked by the stack glow. In Tanimoto's terms Scratch sits at roughly level 4 for edits made while a script is running. Victor argues that liveness alone is not enough, and that the environment must also show state and flow.

### Cited Findings
- A one-page summary of Maloney et al. (2004 and 2010) gives the core principles:
  - "Liveness means code is always running. Tinkerability means even code fragments can be run and experimented with—e.g. click any fragment to run it."
  - "There are no error messages; tile shapes constrains combinations."
  - The overview is a "single window; short, browsable command set; color coded command tiles."
  - Annotated screenshots show: the running block outlined in white, the running command highlighted in yellow (both when single stepping and when running), an expression's result shown on hover, an error shown in red, and "Data is concrete—put it on the stage to see it; data changes are animated." (The red error and the single-stepping marks seem to come from a Squeak-era or experimental build, not shipping Scratch 3.)
  - [Scratch summary, Smalltalk Zoo / CHM](https://smalltalkzoo.computerhistory.org/users/Dan/uploads/Scratch.pdf)
- Clicking a stack in Scratch 3 is a toggle. `runtime.toggleScript` carries the comment "Toggling a script that's already running turns it off". If no thread is running that stack, it pushes a new thread with a `stackClick` flag. A thread started by a click on an edge-activated hat can coexist with the thread that hat starts each frame. — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- The glow is computed each frame from thread state:
  - `_updateGlows` considers only threads on the target being edited (`if (target === this._editingTarget)`).
  - A stack glows if `thread.requestScriptGlowInFrame || thread.stackClick`.
  - It emits `SCRIPT_GLOW_ON` / `SCRIPT_GLOW_OFF` events with the top block id. A separate `BLOCK_GLOW_ON/OFF` exists for individual blocks.
  - So the glow covers the whole stack, not the current block, and is shown only for the sprite currently open in the editor.
  - [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- Re-triggering a hat that is already running depends on the hat's metadata. With `restartExistingThreads`, the running thread is restarted in place: `_restartThread` keeps its position "to ensure 2.0-like execution order". Otherwise the runtime will "give up if any threads with the top block are running". The green flag is the restart kind, so pressing it twice restarts scripts rather than doubling them. — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- Monitors (variable readouts shown on the stage) are re-evaluated every step (`_pushMonitors () { this.monitorBlocks.runAllMonitored(this); }`). The UI is told only when the state differs from the previous frame (`if (!this._prevMonitorState.equals(this._monitorState))` → `MONITORS_UPDATE`). — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- Tanimoto's hierarchy:
  - The original 1990 paper had four levels, "culminating in level 4 ('fully live')", meaning a system that "permits a programmer to edit a program while it is running, and furthermore the system continues the execution immediately and without noticeable" interruption.
  - Level 2: a visual representation that is executable. Level 3: updates driven by edits ("the computer would wait and sometime after the programmer did something, would respond"). Level 4: "the computer wouldn't wait but would keep running the program, modifying the behavior as specified by the programmer as soon as changes were made."
  - The 2013 paper adds level 5, "tactically predictive": the environment predicts likely next edits and runs those variants, which the programmer "can select, inspect, or kill". It also adds level 6, "strategically predictive": inferring larger intent.
  - Tanimoto concedes that "liveness at level 4 is probably not very important in most kinds of programming", for example for programs that terminate in milliseconds.
  - [Tanimoto 2013](https://homes.cs.washington.edu/~tanimoto/site/wp-content/uploads/2024/10/LiveProgramming2013.pdf)
- Critics argue that levels 5 and 6 can exist without the immediate editing liveness the hierarchy treats as a prerequisite. — [Basman et al., PPIG 2016](https://ppig.org/files/2016-PPIG-27th-Basman1.pdf), via search snippet
- Victor, "Learnable Programming":
  - "live coding, as a standalone feature, misses the point."
  - The environment must make "the program flow … visible and tangible" ("Follow the flow").
  - "The entire purpose of code is to manipulate data, and we never see the data" ("See the state", "All state must be eliminated or shown").
  - Learners should "start with something, then adjust until it's right" ("Create by reacting").
  - Learners should "start concrete, start grounded" and then generalize ("Create by abstracting").
  - [Victor, Learnable Programming](https://worrydream.com/LearnableProgramming/)
- Victor criticizes the practice of "randomly adjusting numbers" to discover what unlabeled arguments do, and says labels in context are needed ("Read the vocabulary"). — [Victor, Learnable Programming](https://worrydream.com/LearnableProgramming/)

### Inferences
- Scratch's liveness is cheap because a "program" is just a set of independently clickable stacks and the stage is a persistent world, not a process that is restarted. Clicking a stack is "spawn a thread now on this sprite". Our engine's equivalent would be "inject a one-off command into the live world at the next tick boundary".
- The glow is a per-frame projection of the scheduler state onto the editor. It is cheap, and it is a strong answer to Victor's "follow the flow" at the granularity of a whole stack. Highlighting individual blocks was used for single stepping in older builds.
- Scratch covers Victor's "see the state" through monitors (a checkbox next to each variable in the palette) and the always-visible stage. It covers "follow the flow" only at stack level, and has no history or time scrubbing. A deterministic engine that records input could offer the scrubbing Victor asks for, which Scratch cannot.

### Gaps
- Bret Victor's "Inventing on Principle" talk (2012) was not fetched. Its specific claims (immediate connection between creator and creation, scrubbing time, the "instant feedback" Mario demo) are from general knowledge and are not cited here.
- The exact wording in Maloney 2010 about the single-window layout, the stage always being visible, and editing while running could not be verified from the full text.

## 2. A runtime that fails soft (almost no error messages)

### Takeaway
Scratch 3 deliberately turns every runtime mistake into a defined, harmless value:
- NaN becomes 0.
- An invalid list index becomes a no-op or an empty result.
- Strings compare case-insensitively.
- A small set of strings counts as false.

The cost is invisible bugs. Fraser points out that even the "no syntax errors" claim fails when a block sits next to a stack but is not connected to it.

### Cited Findings
- Maloney et al. state that Scratch does not display error messages, attribute this partly to block shapes that only fit together meaningfully, and acknowledge that dropping error messages does not remove errors. This is reported from search snippets of the SciSpace and Scribd copies; the exact page was not verified. — [Maloney et al. 2010 (SciSpace copy)](https://scispace.com/pdf/the-scratch-programming-language-and-environment-8306uywoc2.pdf); [Wilson & Moffat, PPIG 2010](https://scratched.gse.harvard.edu/sites/default/files/wilson-moffat-ppig2010-final.pdf)
- Coercion rules in scratch-vm's `Cast` ([cast.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/util/cast.js)):
  - `toNumber`: "Scratch treats NaN as 0, when needed as a number". Both the NaN branch and the failed-conversion branch return `0`.
  - `compare`: if both values convert to numbers, the result is `n1 - n2`. Otherwise "Scratch compares strings as case insensitive" (both sides are lowercased). A whitespace string that would convert to 0 is forced to NaN so it compares as a string. Equal infinities compare as 0.
  - `toBoolean`: "These specific strings are treated as false in Scratch": `''`, `'0'` and any casing of `'false'`. "All other strings treated as true."
  - `toListIndex`: indexes start at 1. `'last'` gives the length. `'random'`/`'any'` gives `1 + Math.floor(Math.random() * length)`. `'all'` gives `LIST_ALL` when accepted. Anything out of range (`index < 1 || index > length`) gives `LIST_INVALID`, which the list blocks then treat as a no-op or an empty result.
- Fraser, on how block programming can still hide a syntax error: "One of the great selling features of block-based programming is the lack of syntax errors. But … (from Scratch) has an invisible syntax error: the 'turn' block is not connected to the 'move' block above it." Blockly's fix: "every time a block is moved, look for unattached connector pairs that are ambiguously close to each other and bump one block away from the other." — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Blockly's language-design guidance defaults to lists that start at 1, because "novice programmers react badly when they encounter zero-based lists for the first time". It recommends case-insensitive, permissive variable names. — [Blockly: Block- vs. text-based languages](https://docs.blockly.com/guides/design/languages/), via search snippet

### Inferences
- Why fail soft:
  - (a) Children tinker by poking. A modal error stops the loop of "start with something, then adjust", where a no-op keeps the project running.
  - (b) Scratch scripts are long-lived concurrent threads on a persistent stage. An exception would need an answer to "which thread dies, and what about the others?"
  - (c) There is no console to show errors in.
- The downsides:
  - Errors become wrong behavior that is silent and far from its cause.
  - Learners never form the habit of reading errors, which matters when they move to Python.
  - Coercion makes `"10" < "9"` and `"apple" = "APPLE"` behave differently from Python.
  - The `random` list index uses `Math.random`, an unseeded source of randomness. That is a small determinism leak that the fail-soft vocabulary hides.
- Python is the opposite: `IndexError`, `ZeroDivisionError` and `TypeError` are loud and are raised at the faulting line. A block view of Python must choose between:
  - Translating blocks to fail-soft helpers. That breaks the "blocks are just Python" round trip.
  - Keeping Python semantics and adding an error display that Scratch never had.

  The middle path that mods have taken is to catch the exception per handler, keep the world running, and mark the failing block. That is an inference; no source was found for a specific mod doing this.

### Gaps
- Not verified at the source: Scratch's `x / 0` yields Infinity (JS semantics) and `sqrt(-1)` yields NaN shown as "NaN". This is consistent with `Cast` returning JS numbers, but the operator blocks' code was not read.
- There are no empirical studies in hand measuring the learning cost of the fail-soft runtime.

## 3. Concurrency model, the sequencer and frame timing (scratch-vm)

### Takeaway
Scratch 3 is a cooperative green-thread scheduler. Each frame (60 Hz by default, 30 Hz in "compatibility mode") it runs repeated passes over all threads, in a fixed list order. A non-warp thread yields at the end of each loop iteration and at waits. Passes stop when a redraw is requested (unless in turbo mode) or when 75% of the frame's time budget is used. Warp ("run without screen refresh") loops run for up to 500 ms of wall-clock time before yielding. That time budget makes the number of iterations per frame depend on the machine: Scratch is *not* deterministic in the sense our engine requires.

### Cited Findings
- Frame interval: `THREAD_STEP_INTERVAL` is `1000 / 60`; `THREAD_STEP_INTERVAL_COMPATIBILITY` is `1000 / 30`. `setCompatibilityMode` restarts the interval timer. — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- Order of work in `runtime._step` ([runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)):
  1. Filter out killed threads.
  2. Start edge-activated hats ("For compatibility with Scratch 2" they run first).
  3. Set `redrawRequested = false`.
  4. Call `sequencer.stepThreads()`.
  5. Then `renderer.draw()`.
- In `stepThreads` ([sequencer.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/sequencer.js)):
  - The budget is `WORK_TIME = 0.75 * this.runtime.currentStepTime`.
  - Passes repeat while there are threads, active threads remain, `timer.timeElapsed() < WORK_TIME`, and `(this.runtime.turboMode || !this.runtime.redrawRequested)`.
  - Each pass steps every thread in `runtime.threads` index order. The time check runs between passes, so a pass always completes.
  - Threads that are done or killed are compacted out after the pass, keeping their relative order.
- Turbo mode changes only the redraw condition. It does not change `WORK_TIME`. So turbo means "don't stop at the first visual change; keep iterating until the 75% budget is used". — [scratch-vm sequencer.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/sequencer.js)
- Thread states and yields ([sequencer.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/sequencer.js)):
  - The states are `STATUS_RUNNING`, `STATUS_YIELD`, `STATUS_YIELD_TICK`, `STATUS_PROMISE_WAIT` and `STATUS_DONE`.
  - `YIELD_TICK` (used by waits) lasts at least one tick via `ranFirstTick`.
  - Inside warp mode a yield re-executes immediately while `thread.warpTimer.timeElapsed() <= Sequencer.WARP_TIME`, where `WARP_TIME` is `500` ms.
- Community explanation of the user-visible model: a script runs until it reaches a yield point (the end of a loop iteration, a wait, or the end of the script) and then resumes on its next turn. Code between yields is effectively atomic, which users exploit to avoid races; a race appears only when a loop lacks a yield. These are forum posts, so moderate reliability. — [Scratch forums](https://scratch.mit.edu/discuss/post/3371380), [Scratch forums](https://scratch.mit.edu/discuss/post/2576750)
- Execution order across sprites follows layer order: `executableTargets` holds the "Targets in reverse order of execution. Shares its order with drawables." `allScriptsDo` iterates from the end, and `setExecutablePosition` lets "Infinity … set the target to execute first." So moving a sprite to the front changes the order in which its scripts run. — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)
- There is a hard cap on clones: `MAX_CLONES` is `300`. `clonesAvailable()` checks `_cloneCounter < MAX_CLONES`, and "create clone" silently does nothing past the cap (failing soft again). — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)

### Inferences
- Within one pass, thread order is deterministic: list order, then layer order. What varies between runs is how many passes fit in a frame, because that depends on wall-clock time (`WORK_TIME`, `WARP_TIME`). Two machines can produce different results for the same project when a loop without redraws (or a warp procedure) does work that depends on iteration count. Add the unseeded `Math.random` and the "timer" block, and Scratch is reproducible only "visually, usually".
- The "redraw requested" rule is a clever piece of UX. A `forever { move 10 }` loop animates one step per frame, because the move requests a redraw and ends the frame. A loop that only does arithmetic runs flat out. Novices get animation without thinking about frames. The same script therefore runs at very different speeds depending on whether its body touches the screen. That coupling is a known confusion, and turbo mode exists to break it.
- For us, the one-iteration-per-loop-pass yield maps naturally onto "one loop iteration per tick" (a generator or coroutine resumed once per fixed tick). It must be bounded by an instruction or iteration count, never wall-clock time, to stay deterministic. A warp equivalent would be "run to completion within the tick, with a fixed step cap". When the cap is hit, the behavior must be deterministic, for example a logged error.
- "Broadcast and wait" fits as: fire an event, then suspend the caller until every handler thread started by that event reports done. Scratch's ordering for this was not verified (see Gaps).

### Gaps
- The source for the `broadcast and wait` and `wait` blocks (`scratch3_event.js`, `scratch3_control.js`) was not read. Exact semantics are not verified: for example, whether broadcasting a message that is already running restarts its handlers (believed yes, via `restartExistingThreads`), and how "and wait" detects completion.
- Whether the scratch.mit.edu editor in 2026 still defaults to 60 Hz with a 30 Hz compatibility mode, or the reverse. The VM exposes both, and the GUI's default was not checked. Historically Scratch 2 ran at 30 fps, and Scratch 3's default player runs at 30 fps ("compatibility mode" on by default). That is general knowledge, not verified this session.
- No official Scratch Team statement on determinism was found.

## 4. Drag-and-drop ergonomics (Blockly / scratch-blocks)

### Takeaway
Feel comes from a handful of tuned constants and rules:
- A small radius before a drag starts.
- A larger radius for drags that start in the flyout (to tell drag from scroll).
- A snap radius of about 28 px.
- Hysteresis that keeps the current candidate connection.
- An automatic "bump" that pushes apart blocks that sit close but are not connected.

Fraser's lessons say transitive connections are "a disaster", and that a small visual gap inside C-blocks made stacking discoverable.

### Cited Findings
- Blockly `config` defaults ([Blockly core/config.ts](https://raw.githubusercontent.com/google/blockly/develop/packages/blockly/core/config.ts)):
  - `dragRadius: 5`: "Number of pixels the mouse must move before a drag starts."
  - `flyoutDragRadius: 10`: "Because the drag-intention is determined when this is reached, it is larger than dragRadius so that the drag-direction is clearer."
  - `snapRadius` and `connectingSnapRadius`: `DEFAULT_SNAP_RADIUS = 28`, the "Maximum misalignment between connections for them to snap together."
  - `currentConnectionPreference: 8`: "How much to prefer staying connected to the current connection over moving to a new connection. The current previewed connection is considered to be this much closer … than it actually is."
  - `bumpDelay: 250`: the "Delay in ms between trigger and bumping unconnected block out of alignment."
- Fraser's lessons ([Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)):
  - **Transitive connections:** "dragging a block connected to a block and trying to make a connection on the child block … The transitive closure seems like a good idea (which Blockly implemented). But in practice it is a disaster … users will often drag a large assembly partially off-screen to clear room … the user will often hear a click sound as the assembly randomly connects to something … Transitive connections turn large assemblies into Fluorine atoms they bond with everything."
  - **Nesting sub-stacks:** Scratch nests the first statement snugly inside a C-block with no inside-bottom connector. Fraser says this "appears to combine the worst discoverability attributes of both issues". Blockly found, after "a rendering bug occurred which added the small gap", that "We saw a marked improvement in user studies due to this bug". The reason: "Snug nesting leaves no hint that more blocks can be stacked."
  - **Border style:** designers strip the bevels and shadows, but "these 'outdated decorations' are vital for distinguishing connected blocks that are the same colour."
  - **Bumping:** the fix for disconnected blocks that look connected (quoted in section 2).
  - The click sound on connection is mentioned as existing feedback in passing ("hear a click sound").

### Inferences
- The hysteresis (`currentConnectionPreference`) matters as much as the radius. Without it the insertion preview flickers between two candidates when the cursor sits between them.
- Dragging a block takes everything below it in Scratch and Blockly, which matches "a statement and its continuation" (a suffix of a list). Our block model on `libcst` will need a "drag the tail" operation (a suffix of a statement list) as the default, with a modifier for dragging a single block. In Blockly, a block can be healed out of the middle of a stack with a modifier; that is from general knowledge, not verified.
- Fraser's "Fluorine atoms" lesson argues for connecting only the dragged root's own connections, never connections of blocks nested inside the dragged assembly.

### Gaps
The following are from general knowledge and were not verified at a primary source in this session:
- Drag to the palette or flyout to delete. Blockly treats the toolbox and flyout as a delete area, alongside the trashcan.
- The Scratch backpack: per-user storage across projects, on scratch.mit.edu only.
- The right-click workspace menu: "Clean up blocks" (arranges top-level stacks in a column), "Add comment", "Undo/Redo", "Delete N blocks" with a confirmation.
- "Duplicate", which in Scratch attaches the copy to the cursor and copies the stack below too.
- Insertion markers: the grey ghost of the dragged block shown at the candidate position in scratch-blocks and Blockly's insertion-marker previewer.

scratch-blocks constants (for example its snap radius, believed to be 48 at Scratch's larger block scale) were not retrievable: the old `core/constants.js` path returned 404, and the repo layout appears to have changed. The repo is active (pushed 2026-11-14, default branch `develop`) per the [GitHub API](https://api.github.com/repos/scratchfoundation/scratch-blocks).

## 5. Discoverability: categories, plain-language labels, default arguments, local and global variables, monitors, the sprite as an object

### Takeaway
Discoverability comes from showing everything: every block is visible, labeled in plain language, colored by category, and pre-filled with values that work. Fraser's lessons add two points: color and category must separate concepts that confuse learners (conditionals vs loops), and written instructions do not work.

### Cited Findings
- "The most difficult blocks for new users are conditionals and loops. Many block environments add to the difficulty by making both blocks (already similarly shaped) the same colour and placing them in the same category … Blockly moved conditionals into the logic group, changed the colour, and the problem went away." — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Instructions: "Students would not read them. We reduced them to a single sentence, increased the font size, and highlighted them in a yellow bubble. Students would not read them. We created modal popups … Students instinctively closed the popups without reading them, then were lost." What finally worked was "popups that cannot be closed … programmed to monitor the student's actions and only close themselves when the student has performed the proscribed action." — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- "Your other left": US children confused left and right. Arrows fixed it, and for directions relative to an avatar "Most helpful is a circular arrow". — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Code ownership: non-editable, fill-in-the-blank starter blocks backfired ("students hated these … They have no sense of ownership"). Using "the student's own solution for one exercise as the starting point for the next" worked. — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Live block images: documentation embeds read-only Blockly instances instead of screenshots, because 50 images × 50 languages means "2,500 static images". "Readonly mode has made internationalization possible." — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Victor: autocomplete should insert working default arguments so that each completion shows a result and the learner can "get something on the screen as soon as possible". He also asks to "Dump the parts bucket onto the floor", meaning show every available part beside the code. — [Victor, Learnable Programming](https://worrydream.com/LearnableProgramming/)
- The Scratch summary lists a "short, browsable command set; color coded command tiles" and "Data is concrete—put it on the stage to see it". — [Scratch summary](https://smalltalkzoo.computerhistory.org/users/Dan/uploads/Scratch.pdf)
- Monitors are re-evaluated every frame and pushed only when they change (section 1). — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)

### Inferences
- Default values in sockets (`move (10) steps`, `turn (15) degrees`) make a block dropped from the palette do something visible when clicked. They are Victor's "default argument" principle applied, and they are what makes click-to-run useful. Our palette from reflection needs a default value per parameter, ideally chosen to be visible, not just a type default of 0.
- Scratch makes "for this sprite only" vs "for all sprites" a choice in the create-variable dialog. That maps to component state on the entity vs a singleton or global in our ECS. The dialog's wording is a cheap way to teach scoping.
- The monitor checkbox next to each variable in the palette is a one-click "see the state". For us, a monitor reads a component field each frame on the presentation side. It is read-only, so it does not affect determinism.

### Gaps
- The exact Scratch 3 category colors and the rationale for them, and Scratch's sprite-local vs global dialog text, were not verified at a source this session.

## 6. Media and first run

### Takeaway
No primary source was gathered on this in this session. What follows is flagged as general knowledge for the report writer to verify or drop.

### Cited Findings
- Blockly Games' "exit strategy", which bears on first run and graduation ([Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)):
  1. Block text is lowercase "to match text-based programming languages".
  2. "The JavaScript version of the student's code is always displayed after each level."
  3. In the second-to-last game "the block text is replaced with actual JavaScript".
  4. In the last game the block editor becomes a text editor.

  Fraser: "Nobody ever got a job because they had three years of Scratch experience."

### Inferences
- A new Scratch project opens with a sprite (the cat) already on the stage. So the very first drag-and-click (`move 10 steps`) produces visible motion within seconds. The asset (the sprite) exists before any code. The equivalent for our engine is a starter project with an entity that has a sprite and a Position, plus a palette whose top blocks act on that entity.

### Gaps
Not researched with sources:
- The Scratch costume and sound editors.
- The sprite, backdrop and sound libraries.
- The tutorials pane (introduced with Scratch 3 in 2019).
- Starter projects.
- Any measurement of time to first result.

## 7. Accessibility and localization

### Takeaway
Blockly has moved from "inaccessible" to "keyboard navigable and screen-reader accessible by default". The keyboard navigation plugin shipped in May 2025, and v13 folded it into core, funded by a $5M Google.org fund. Research systems (Blocks4All) found that blind children need blocks described fully by the screen reader, a predictable place for the palette, and program output that is not visual.

### Cited Findings
- Blockly accessibility ([Blockly accessibility](https://www.blockly.com/accessibility); [Blockly v13 announcement](https://groups.google.com/g/blockly-announce/c/U9-gdZN5NvM)):
  - Google.org launched a $5 million Blockly Accessibility Fund in May 2024.
  - Blockly v12 and the Keyboard Navigation plugin were published in May 2025.
  - Q3 2025 work was planned on screen readers and "ARIA attributes for fields and workspace components".
  - v13: "Blockly fully supports keyboard navigation controls, enabled and on by default". Supported screen readers are NVDA, JAWS, Narrator, VoiceOver and ChromeVox, plus refreshable braille displays. Plugin users should uninstall the plugin when upgrading.
- One Blockly docs page still lists a target of June 2026 for this code. It is likely stale given the v13 announcement. — [Blockly keyboard navigation docs](https://docs.blockly.com/guides/configure/keyboard-nav/), via search snippet; conflicts with [v13 announcement](https://groups.google.com/g/blockly-announce/c/U9-gdZN5NvM)
- Code.org documents keyboard navigation and screen reader support in its Blockly labs. — [Code.org support](https://support.code.org/hc/en-us/articles/47623117066765-Keyboard-Navigation-and-Screen-Reader-Support-in-CodeAI-Blockly-Labs)
- Blocks4All (Milne & Ladner, CHI 2018) ([UW DO-IT](https://www.washington.edu/doit/blocks4all-five-keys-accessible-block-based-languages), via search snippet; paper: [Milne & Ladner](https://milnel2.github.io/files/Blocks4All.pdf)):
  - It is an iPad app, chosen for the built-in VoiceOver and zoom.
  - "Blocks are fully described (title, description, location)" by VoiceOver.
  - Blocks are placed at the bottom of the screen so they can be found without vision.
  - The program's output is a robot, so only a run button is needed on screen.
- Localization: Fraser's live read-only block images are what "made internationalization possible". — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)

### Inferences
- Blocks4All's "program output is a robot" generalizes: a visual stage is itself an accessibility barrier. Our accessibility tree (`sage_ui` `access`) should cover the game view's state (for example entity names, positions and monitor values), not only the editor chrome.
- Translating block text implies that block labels are templates with argument slots that can be reordered (`%1`-style), not concatenated strings. Right-to-left languages also mirror the layout of the block (where the connection notch sits, the direction of flow). Both are standard in Blockly per general knowledge (its `Blockly.Msg` tables and the workspace `rtl` option) but were not verified this session.

### Gaps
- Not fetched: the Pause-Pause paper, and the details of Scratch's own accessibility work (Scratch 3 has historically had limited screen-reader support).
- High-contrast themes and colorblind analyses of block palettes were not researched. Blockly ships a high-contrast theme per general knowledge.
- The Blockly v13 keyboard model was not read in detail (move mode, cursor over the AST, shortcut map).

## 8. Scaling: what breaks in big projects

### Takeaway
Scratch is weak at scale:
- No search across scripts.
- Weak abstraction: custom blocks cannot return values.
- No version control.
- A cap on project size.

Evidence here is mostly community complaints. The mitigations of Snap! and MakeCode were not researched with sources this session.

### Cited Findings
- Users report that with many scripts in one sprite "it is hard to find them". They report difficulty seeing "what calls what" and name collisions from copy-paste. They want source control ("it would be very useful to have some form of source control (Git, etc)"). Large scripts make the editor lag. These are forum posts, so the evidence is anecdotal. — [Scratch forums](https://scratch.mit.edu/discuss/post/1096982), [Scratch forums](https://scratch.mit.edu/discuss/post/6982035), [Scratch forums](https://scratch.mit.edu/discuss/post/7717279/)
- Custom blocks in Scratch "only allow you to execute code and not return values", with a limited set of parameter types. — [Scratch forums](https://scratch.mit.edu/discuss/post/405030), via search snippet
- A 5 MB limit on project JSON is reported. It is community-reported and was not verified at the source. — [Scratch forums](https://scratch.mit.edu/discuss/post/393651), via search snippet
- Fraser: transitive connections become worse as programs grow, because users drag large assemblies off screen to make room. — [Fraser 2015](https://docs.blockly.com/publications/papers/TenThingsWeveLearnedFromBlockly.pdf)
- Clone cap of 300 and silent failure past it. — [scratch-vm runtime.js](https://raw.githubusercontent.com/scratchfoundation/scratch-vm/develop/src/engine/runtime.js)

### Inferences
- A block view that is "a view of plain Python" sidesteps the worst of these: version control, grep and refactoring happen on the text. Text diffs of block edits are only readable if the round trip keeps formatting, which `libcst` is chosen for.
- Fraser's "exit strategy" (always show the generated text, then swap the block labels for code) is a ready-made plan for a block view that is already Python.

### Gaps
Not researched with sources:
- Snap! (first-class procedures and lists, custom reporters, a "find blocks" search, "unused blocks" cleanup).
- MakeCode (switching between Blocks, JavaScript and Python on the same project, text as the source of truth, GitHub integration).
- Scratch mods: TurboWarp's compiler, which makes projects faster but changes timing; its "Find bar" add-on; and Scratch Addons' search features.

All of these are from general knowledge only.

## Implications for our engine

These are inferences from the above, in tension with determinism and Python semantics.

1. **Clicking to run a stack** should be "spawn a handler now". In our model it must enter as *input*: a recorded command applied at the next tick boundary. That way replays reproduce it, and wall-clock time never reaches the simulation. The glow is a presentation-only projection of the scheduler's per-tick state, sent through `Frame`/`UiFrame` the way scratch-vm emits `SCRIPT_GLOW_ON/OFF`.
2. **Yield points.** Scratch-style loops ("one iteration per frame") map onto coroutines resumed once per tick in StableId order. Scratch bounds work with wall clock (`WORK_TIME = 0.75 × frame`, `WARP_TIME = 500 ms`), and that is exactly the nondeterminism we must not copy. Use iteration or instruction caps that count deterministically. The cost: the "redraw requested" rule that makes Scratch animate for free has no direct equivalent unless we define "visual change" as a deterministic event.
3. **Fail soft vs Python.** Scratch's `NaN → 0`, no-op list indexes and case-insensitive comparison contradict Python and would break the "blocks are just Python" round trip if built into the blocks. Proposal: keep Python semantics. Catch exceptions per handler call, deterministically: same tick, same handler, same outcome, so this works under replay. Mark the failing block in the editor, and keep the world running. Avoid Scratch's unseeded `random` list index; all randomness goes through the world's RNG.
4. **Execution order.** Scratch orders threads by layer (render order), so bringing a sprite to the front changes what its scripts do. Our StableId order is better: stable and independent of presentation. It should be shown to users, because order matters with yields.
5. **Edit while running** (Tanimoto level 4) works with our live reload plus snapshot. Determinism adds a requirement: a code edit is an input event at a tick. Otherwise a recording cannot reproduce a session that included edits.
6. **Drag feel.** Adopt Blockly's constants as starting values:
   - Drag radius 5, larger in the palette (10).
   - Snap radius about 28 px at Blockly's scale.
   - Hysteresis toward the current candidate.
   - A 250 ms bump that separates blocks that sit close but are not connected.
   - Never connect transitively.
   - Leave a visible gap at the bottom of C-blocks.

   Bumping matters doubly for us, because an unconnected block silently becomes a separate top-level statement or dead code in the Python text.
7. **Default arguments.** Every palette entry from reflection needs a default that works and is visible, so a freshly dropped block does something when clicked.
8. **Monitors** are free in an ECS: each frame, a presentation-side read of a component field. They answer Victor's "see the state". A recorded deterministic simulation can also offer the time scrubbing Victor asks for, which Scratch cannot.
9. **Instructions and tutorials.** Fraser's evidence says text instructions get ignored. Prefer tutorial steps that watch for an action, implemented as editor-side observers.
10. **Accessibility** should extend to the stage (game state in the accessibility tree), per Blocks4All. Block labels should be templates that can be reordered and mirrored, to support translation and right-to-left languages.
