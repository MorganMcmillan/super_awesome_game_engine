# HCI principles and evidence for complex software that serves every skill level

Scope: cross-cutting theory and empirical evidence (not product case studies). Evidence labels used throughout:
- **[Strong]**: controlled experiment(s) or field data with replication or a large sample.
- **[Moderate]**: one controlled study, small-N field study, or a vendor's internal data reported by the vendor.
- **[Opinion]**: expert opinion, design framework or argument, no direct test.
- **(verified)** means the claim was checked against the primary source or its abstract during this research session. **(not re-fetched)** means it comes from well-known literature whose citation is standard but whose exact numbers I did not re-check this session. The report writer should treat those as needing a citation check before quoting numbers.

## Novice-to-expert transitions (dips and ceilings, marking menus, shortcuts, paradox of the active user, perpetual intermediates)

### Takeaway
Users stop improving well short of the performance ceiling. They are not lazy: switching to a faster method costs them a temporary "performance dip", so they satisfice. Designs that make the expert method *the same gesture* as the novice method (marking menus, rehearsal), or that make the novice path slightly costlier or more informative (audio feedback, showing the shortcut), measurably speed the transition. Most users spend most of their time as "intermediates", so the intermediate experience matters most.

### Cited Findings
- Scarr, Cockburn, Gutwin and Quinn define the framework: interfaces have a performance *ceiling*; moving from a novice modality (menus) to an expert one (hotkeys, gestures) causes a *dip* in performance that users avoid, which leaves them stuck on a lower curve. Published CHI 2011, pp. 2741-2750, DOI 10.1145/1978942.1979348 (verified) — [researchr](https://researchr.org/publication/ScarrCGQ11); [DOI](https://doi.org/10.1145/1978942.1979348). [Opinion/framework, built on prior experiments]
- Cockburn, Gutwin, Scarr and Malacria's survey "Supporting Novice to Expert Transitions in User Interfaces" (ACM Computing Surveys 47(2), 2014, DOI 10.1145/2659796) opens by noting that guidelines tell designers to give experts fast options, yet many expert features go unused. It groups performance gains into four domains: *intramodal* improvement (getting faster in the same modality), *intermodal* improvement (switching to a faster modality, e.g. menus to hotkeys), *vocabulary extension* (learning more commands), and *task mapping* (learning better ways to do a task with the commands you know) (verified, abstract) — [dblp](https://dblp.org/rec/journals/csur/CockburnGSM14); [DOI](https://doi.org/10.1145/2659796). Note: the task brief calls this the "Improving command selection" survey; the actual title is "Supporting Novice to Expert Transitions in User Interfaces". "Improving command selection with CommandMaps" is a different paper (Scarr, Cockburn, Gutwin, Bunt, CHI 2012; follow-up "The usability of CommandMaps in realistic tasks", CHI 2014, pp. 2241-2250) — [dblp](https://dblp.org/rec/conf/chi/ScarrCGBC14.html).
- CommandMaps: showing the whole command set on one spatially stable screen (instead of ribbon tabs) lets users exploit spatial memory and was faster than the ribbon for experienced users in the 2012 study (not re-fetched for numbers) — [Academia copy](https://www.academia.edu/19295706/Improving_command_selection_with_CommandMaps). [Moderate]
- Lane, Napier, Peres and Sandor, "Hidden Costs of Graphical User Interfaces: Failure to Make the Transition from Menus and Icon Toolbars to Keyboard Shortcuts", IJHCI 18(2), 2005, pp. 133-144 (citation verified) — [dblp](https://dblp.uni-trier.de/rec/journals/ijhci/LaneNPS05.html). Their result (not re-fetched): even frequent, experienced Word users mostly kept using menus and toolbars for commands they used often, and shortcuts would have been faster. [Strong-ish field/lab data, frequently replicated in spirit]. An unofficial summary quotes savings of roughly 0.66 to 1.48 s per shortcut use depending on the user's default method; treat as unverified — [Notion summary](https://levelshealth.notion.site/Shortcut-All-the-Things-Why-We-Emphasize-Keyboard-Shortcuts-7d6c07b4a3d3443c8879c2a770ef9a1a).
- Grossman, Dragicevic and Balakrishnan, "Strategies for Accelerating On-line Learning of Hotkeys", CHI 2007 pp. 1591-1600 (verified from full text) — [PDF](https://www.dgp.toronto.edu/~ravin/papers/chi2007_learninghotkeys.pdf):
  - Displaying the hotkey label beside the menu item (the standard approach) is commonly ignored by users.
  - Pilot (5 techniques): hotkey use was 70% with audio feedback (speaking the hotkey on menu selection), 69.2% with disabled menu items (menu shows the hotkey but won't execute), 59.1% with a delay, 57.1% control, 50% with extra visual feedback.
  - Main experiment: hotkey use was 28.9% control, 66.6% audio, 72.8% disabled; i.e. more than a 100% increase. Lesson: *cost* (making the slow path a bit slower) and *a different sensory channel* work; more visual decoration does not. [Strong, controlled lab]
- Kurtenbach and Buxton, "User Learning and Performance with Marking Menus", CHI 1994, pp. 258-264 (verified, abstract) — [Autodesk Research](https://research.autodesk.com/publications/user-learning-and-performance-with-marking-menus); [Buxton](https://billbuxton.com/MMUserLearn.html):
  - Marking menus show a radial menu after a short press-and-wait; drawing the same stroke without waiting selects directly. The novice action *is a rehearsal* of the expert action, so there is no modality switch and so no dip.
  - In real work, experts used marks extensively; a mark was on average 3.5 times faster than selecting from the menu; experts still popped the menu up sometimes to refresh their memory.
  - Caveat: the field study had only 2 users and one application — [Berkeley summary](https://people.eecs.berkeley.edu/~fox/summaries/ui/marking_menus.html). [Moderate; the principle was later replicated in many lab studies, see the 2014 survey]
- Carroll and Rosson, "Paradox of the active user" (1987, chapter in *Interfacing Thought*, MIT Press): users want to get work done now, so they skip manuals and training and keep using known, inefficient methods ("production bias"), and they interpret new systems through what they already know ("assimilation bias") (bias names not re-fetched; secondary summary) — [Oulasvirta thread summarizing it](https://threadreaderapp.com/thread/1055166206108467200.html). Fu and Gray (2004, *Cognitive Science*) re-analysed the paradox as *rational* behaviour with a short time horizon: the known method is reliable and general, the new method's payoff is uncertain — same source. [Opinion/observational, very influential]
- Alan Cooper, *About Face* (all editions, Wiley): most users neither stay beginners nor become experts; they become "perpetual intermediates". Beginners want to stop being beginners quickly; experts are few; so design for intermediates, with "training wheels" that come off for beginners and shortcuts for experts. (Book; not fetched this session — search returned nothing usable.) [Opinion]

### Inferences
- The highest-leverage pattern is "novice action rehearses expert action": a palette entry that shows its hotkey, a right-click radial menu that can be flicked, a block that shows its Python text. For this engine the block editor being "a view of plain Python" is exactly a rehearsal design: every block drag is a demonstration of the Python it writes, if the text is visible.
- Do not rely on printed shortcut labels alone; Grossman 2007 shows they are ignored. Cheaper wins: a short, unobtrusive "Ctrl+D" toast/flash when a command is invoked by mouse (the visual-only version did *not* help in the study, so it should be paired with something users notice, or be opt-in audio).
- Accept that most users plateau. Make the plateau high: discoverable vocabulary extension (what else can I do?) matters as much as speed.

### Gaps
- Exact Lane et al. 2005 numbers were not retrieved (paywalled).
- The Carroll and Rosson chapter itself was not fetched; the bias terms are standard in the literature but were not confirmed from primary text in this session.
- Cooper's "perpetual intermediates" was not confirmed from the book in this session (it is in *About Face 3*, ch. 3, from memory).

## Progressive disclosure and layered interfaces (multi-layer, adaptive vs adaptable, Office personalized menus, Clippy)

### Takeaway
Reducing what beginners see helps them, but the *user* should control the reduction. Adaptable (user-chosen) interfaces are preferred and are at least as fast as static ones; adaptive (system-guessed) menus are slower and disliked because they make layout unpredictable. Hiding features also has a cost: users are less aware of what exists.

### Cited Findings
- Shneiderman, "Promoting Universal Usability with Multi-Layer Interface Design", ACM CUU 2003, pp. 1-8: novices start with a small feature set at layer 1 and move up layers when ready; illustrated with a word processor with 8 layers and a map with 3 layers. The DOI is listed inconsistently (10.1145/957205.957206 vs 10.1145/960201.957206) (verified, abstract) — [PDF](https://www.cs.umd.edu/~ben/ACM-CUU2003.pdf); [UMD repository](https://drum.lib.umd.edu/items/90921200-6f02-4f5b-904a-34b87dba9895). [Opinion/design argument]
- Findlater and McGrenere, "A Comparison of Static, Adaptive, and Adaptable Menus", CHI 2004, pp. 89-96, DOI 10.1145/985692.985704: 27 participants; split menus whose top 4 items were static, user-adjustable, or adapted to frequency/recency. Static was significantly faster than adaptive; adaptable was faster than adaptive under some conditions; most participants preferred adaptable (verified, abstract) — [PDF](https://www.cs.ubc.ca/labs/edapt/papers/findlater2004.pdf). [Strong, controlled]
- McGrenere, Baecker and Booth (CHI 2002), field study comparing Word 2000's adaptive "personalized menus" against a version where users could switch between a self-built personal interface and the full interface: users valued and used their personalized interface more than the factory one (verified, secondary summary) — [PDF](https://www.cs.ubc.ca/~ksbooth/PUB/docs/JMcGrenere2002.pdf). [Strong, field]
- Office 2000's adaptively shortened "personalized menus" hid rarely used items; a later study found the assumption that they help selection "is weakened"; Gajos et al. note anecdotal evidence that Office's Smart Menus "inspire strong reactions" while the adaptive Start Menu does not — [Gajos et al. AVI 2006](https://www.eecs.harvard.edu/~kgajos/papers/2006/kgajos-avi06.pdf); [Gajos et al. AI Magazine 2009](https://kgajos.seas.harvard.edu/papers/AIMag09-AUIs.pdf). Gajos's design-space work finds adaptive UIs help only when the adaptation is *predictable* and the accuracy high, and that spatial stability matters (not re-fetched for numbers). [Strong, several controlled studies]
- Findlater and McGrenere also report that reduced-functionality interfaces trade speed for *awareness*: users of a reduced interface know fewer of the features that exist (INTERACT 2007, "Evaluating reduced-functionality interfaces according to feature findability and awareness"; not re-fetched). [Moderate]
- Carroll and Carrithers' "Training Wheels" word processor (1984, CACM) blocked advanced functions for beginners; beginners finished tasks faster and with fewer errors (not re-fetched; classic result). [Strong historically]
- Clippy (Office Assistant): no primary study was found this session. Widely reported as disliked; Microsoft removed it by default in Office XP. Treat as anecdotal. [Opinion]

### Inferences
- Use layers the user picks (e.g. "Beginner / Standard / Everything" or per-panel "show advanced"), never menus that reorder themselves.
- Pair any reduced mode with a way to see what is hidden (search across all commands, a "more" affordance), to avoid the awareness cost.
- Mixed-initiative suggestions (Clippy-like hints) need to be predictable, rare and dismissible forever.

### Gaps
- No primary Microsoft data on Clippy or personalized menus was found.
- No study of layered interfaces specifically for programming or game editors was found.

## Command search and command palettes

### Takeaway
Command search is now standard (VS Code, Sublime, Blender F3, Office "Tell me"/Search, macOS Help search), but peer-reviewed evidence on its effect on discovery is thin. What exists suggests search helps people who know the *name* or *intent* and fails on vocabulary mismatch; it complements, not replaces, visible structure.

### Cited Findings
- I found no peer-reviewed evaluation of VS Code's, Sublime's or Blender's palettes, or of Office "Tell me", in this session. Industry posts on palettes are marketing and say the main challenge is that users must first *know the palette exists* — [CommandBar blog](https://helphub.commandbar.com/blog/command-palette-past-present-and-future). [Opinion]
- Vocabulary problem: Furnas, Landauer, Gomez and Dumais, "The vocabulary problem in human-system communication" (CACM 30(11), 1987): two people choose the same name for an object less than 20% of the time; many aliases per command are needed for good hit rates (not re-fetched; classic, DOI 10.1145/32206.32212). [Strong]
- Grossman, Fitzmaurice and Attar list "unaware of functionality" and "unable to locate functionality" among the main learnability problems found in AutoCAD users (see onboarding below) — search directly targets the second. [Moderate]
- Related Autodesk research, e.g. CommunityCommands (Matejka, Li, Grossman, Fitzmaurice, UIST 2009), recommends commands from other users' usage logs to grow vocabulary, which addresses commands users don't know to search for (not re-fetched). [Moderate]

### Inferences
- A palette is cheap and well-loved by experts; to make it serve beginners: synonyms/aliases (Furnas), show the hotkey and the menu path of each result (so search teaches the location and the faster method), include settings and help topics, and show a "recently used" list when empty.
- In this engine, palette entries can also show the Python call they correspond to, which extends the "rehearsal" idea to scripting.

### Gaps
- Primary evidence on palette discovery effects. Fourney's command-search work (CommandSpace) was not found by search this session.

## The Office ribbon (2007) and large command sets

### Takeaway
The ribbon was a data-driven response to a command set too big for menus and toolbars: Microsoft used opt-in usage logs and longitudinal studies. Its rationale is well documented by Jensen Harris; independent evidence on its benefit is mixed, and it trades expert speed (spatial memory across tabs) for discoverability.

### Cited Findings
- Jensen Harris's "Why the UI" blog series (MSDN archive) explains the redesign. Part 7 ("No Distaste for Paste") uses opt-in Word 2003 usage logs (the Customer Experience Improvement Program): Paste is the most used command, and usage falls off along a long tail where the gap between command #100 and #400 is as large as between #1 and #11; the team concluded the long tail matters, not just the top ten — [MSDN archive](https://learn.microsoft.com/en-us/archive/blogs/jensenh/no-distaste-for-paste-why-the-ui-part-7). [Moderate, vendor data]
- The team ran longitudinal studies on Beta 1 to test whether new UI features were *learnable*, not only discoverable — [MSDN archive](https://learn.microsoft.com/vi-vn/archive/blogs/cbortlik/so-thats-why-office-2007-looks-different). Harris described iterating through rejected prototypes and being "willing to be wrong" — [MSDN archive](https://learn.microsoft.com/en-us/archive/blogs/jensenh/be-willing-to-be-wrong). [Moderate]
- Widely repeated Harris claim (not re-fetched): most feature requests Microsoft received were for features *already in Office*, i.e. the problem was discoverability, not capability. [Moderate, vendor claim]
- Reception was disputed: Microsoft said data showed a small learning curve; reviewers disagreed from experience — [Computerworld](https://www.computerworld.com/article/1477318/migration-productivity-and-sine-language.html). [Opinion]
- CommandMaps (above) showed that for experienced users a single stable command map was faster than the ribbon's tabs, i.e. tab switching taxes spatial memory — [dblp](https://dblp.org/rec/conf/chi/ScarrCGBC14.html). [Moderate]

### Inferences
- Lessons for an editor: gather (opt-in, local) usage data; organise by *task* not by implementation; show labels and large icons for common commands; keep layout spatially stable; keep keyboard access (the ribbon added KeyTips) and a search box.

### Gaps
- No independent, peer-reviewed controlled comparison of the ribbon vs Office 2003 menus with general users was found this session.

## Direct manipulation, gulfs of execution and evaluation, Cognitive Dimensions

### Takeaway
Direct manipulation (visible objects, incremental reversible actions, immediate feedback) narrows Norman's gulfs and suits novices; notations (code, blocks) are better for abstraction and repetition. The Cognitive Dimensions framework is the standard vocabulary for judging the trade-offs of an editor's notation.

### Cited Findings
- Shneiderman, "Direct Manipulation: A Step Beyond Programming Languages", IEEE Computer 16(8), 1983 (DOI 10.1109/MC.1983.1654471): continuous representation of objects; physical actions instead of syntax; rapid, incremental, reversible operations with immediately visible effect (not re-fetched; canonical). [Opinion/framework]
- Hutchins, Hollan and Norman, "Direct Manipulation Interfaces" (HCI 1(4), 1985), and Norman's *The Design of Everyday Things*: the gulf of execution (from intention to action) and the gulf of evaluation (from system state to understanding); direct manipulation reduces both through "directness", but limits abstraction (not re-fetched). [Opinion/framework]
- Green and Petre, "Usability Analysis of Visual Programming Environments: A 'Cognitive Dimensions' Framework", J. Visual Languages and Computing 7(2), 1996 (DOI 10.1006/jvlc.1996.0009): dimensions include viscosity (resistance to change), hidden dependencies, premature commitment, progressive evaluation, visibility/juxtaposability, error-proneness, hard mental operations, role-expressiveness, secondary notation, abstraction gradient, closeness of mapping, consistency, diffuseness, provisionality. Their study found visual languages (LabVIEW, Prograph) were *not* uniformly easier than text; they had high viscosity and secondary-notation problems (not re-fetched). [Framework plus small empirical comparisons]

### Inferences
- Use CDs as a checklist when designing the block view vs text view: blocks lower error-proneness and premature commitment for beginners but raise viscosity and diffuseness for experts; keeping the text view authoritative lets each user pick the trade-off.
- For an ECS: "hidden dependencies" (which systems touch which component) is the classic CD failure; an inspector that shows readers/writers of a component addresses it.
- A deterministic simulation enables "progressive evaluation" (run, rewind, re-run identically), which CDs prize.

### Gaps
- No CD study of a game editor specifically was retrieved.

## Undo models and exploration

### Takeaway
Reliable undo is the precondition for exploratory learning; users explore more when they can undo. Linear undo is universal; selective undo and visual history help experts but are harder to make predictable.

### Cited Findings
- Myers et al., "Selective Undo Support for Painting Applications" (Aquamarine), CHI 2015, DOI 10.1145/2702123.2702543: implements selective undo with a *script model* (re-execute history skipping the undone op) rather than inverse operations; two informal studies found it usable and desirable. Its related work states "studies have shown that when users have the ability to undo, they are more comfortable exploring and trying new commands", citing Myers and Kosbie 1996 (verified via summary) — [core.ac.uk PDF](https://core.ac.uk/download/357621637.pdf); [Aquamarine project](https://www.cs.cmu.edu/~marmalade/aquamarine.html). [Moderate; the exploration claim is second-hand]
- Kurlander's editable graphical histories (1988) and "A visual language for browsing, undoing, and redoing graphical interface commands" (1990) introduced history as a browsable, editable object — [HCI museum: Selective Undo](https://hci-museum.lisn.upsaclay.fr/selective-undo). Su et al. (MIT) argue that linear histories restrict users to sequential exploration and propose visual, branching histories — [PDF](https://people.csail.mit.edu/sparis/publi/2009/mittr/Su_09_Interactive_Visual_Histories.pdf). [Moderate]
- Shneiderman's direct-manipulation principles include "reversible" actions as a core property (see above). Rieman's diary studies of exploratory learning (1996) found users learn new features mostly by trying them in context (not re-fetched). [Moderate]

### Inferences
- The engine's editor already stores undo as scene differences and has a deterministic simulation with snapshots; this supports "undo everything, including play-mode experiments" and branching history (try two variants and compare). Make undo cover *every* edit including block and script edits, or beginners will stop trusting it.
- Selective undo should be an expert feature behind a history panel; default to linear undo.

### Gaps
- No controlled study directly measuring exploration rate with vs without undo was retrieved; the claim rests on Myers' citation.

## Onboarding: templates, tutorials, contextual help, minimalist instruction

### Takeaway
The best-supported approaches put help *at the point of need* and let users act immediately: contextual video tooltips (ToolClips), minimalist task-oriented instruction, and working examples/templates. Long up-front tutorials are skipped (paradox of the active user).

### Cited Findings
- Grossman, Fitzmaurice and Attar, "A Survey of Software Learnability: Metrics, Methodologies and Guidelines", CHI 2009 pp. 649-658, DOI 10.1145/1518701.1518803: no agreed definition of learnability across 88 papers (1982-2008); separates *initial* learnability from *extended* learnability (getting better over time); introduces a question-suggestion protocol (an expert sits with the user and suggests better methods) that found more learnability problems than think-aloud; classifies problems (e.g. understanding a task flow, awareness of functionality, locating functionality, understanding functionality, transitioning to efficient behaviours) (verified, abstract; categories from memory) — [Autodesk PDF](https://research.autodesk.com/app/uploads/2023/03/a-survey-of-software.pdf_recqWPTEYd6nsP0r8.pdf). [Strong]
- Grossman and Fitzmaurice, "ToolClips: An Investigation of Contextual Video Assistance for Functionality Understanding", CHI 2010: tooltips extended with short videos and text; users integrated them into their tasks; users completed 7 times as many unfamiliar tasks as with a professional online help system, and were significantly faster a week later (verified, abstract; sample size not checked) — [Autodesk PDF](https://research.autodesk.com/app/uploads/2023/03/toolclips-an-investigation-of.pdf_recbnW20cbxDanglq.pdf). [Strong]
- Carroll, *The Nurnberg Funnel: Designing Minimalist Instruction for Practical Computer Skill* (MIT Press, 1990): instruction that lets learners start real tasks at once, cuts reading, supports error recognition and recovery, and exploits learners' prior knowledge; minimal manuals produced faster learning than conventional manuals in Carroll's studies (not re-fetched). [Strong historically]
- Carroll and Rosson 1987 (above): users skip manuals and tutorials to get to work. [Observational]

### Inferences
- Onboarding for this engine: start from a runnable template game (Pong already exists), with a small number of in-context "do this next" prompts; tooltips with a 5-second clip or animated preview for panels and blocks; error messages that link to the exact fix. Every tutorial step should be an edit to a real project, not a slideshow.
- Measure *extended* learnability, not just first-hour success: track whether intermediate users discover hotkeys, the palette, scripting.

### Gaps
- Comparative evidence for interactive step-by-step tutorials vs templates in creative tools was not retrieved this session.

## Customization: workspaces, rebindable keys, and how often users customize

### Takeaway
Most users customize little; the main barriers are time and not knowing how. Customization rises when it is visible, cheap, and socially prompted. Customization is therefore an expert/power feature that must still be easy to find and share.

### Cited Findings
- Mackay, "Triggers and Barriers to Customizing Software", CHI 1991 pp. 153-160, DOI 10.1145/108844.108867: 51 users at MIT Project Athena over four months. Triggers: noticing one's own repeated patterns, fixing something annoying or slow, making new versions behave like old ones; barriers: lack of time (most cited, about two-thirds) and difficulty/not knowing how (about a third). Customizations spread socially through shared files (verified, abstract/secondary) — [PDF](https://www.lri.fr/~mackay/pdffiles/CHI91.Triggers.pdf). [Strong for its time, field]
- Banovic, Chevalier, Grossman and Fitzmaurice, "Triggering Triggers and Burying Barriers to Customizing Software", CHI 2012 pp. 2717-2726, DOI 10.1145/2207676.2208666: increasing exposure to and awareness of customization features, and adding social influence, significantly changed users' customization behaviour; users develop different strategies (verified, abstract) — [Autodesk PDF](https://research.autodesk.com/app/uploads/2023/03/triggering-triggers-and-burying.pdf_recQmDHm7lgNUZAhp.pdf). [Strong, controlled]
- Page, Johnsgard, Albert and Allen (CHI 1996, "User customization of a word processor") found that a large majority of WordPerfect users made some customization, typically small, when customization was easy (not re-fetched; percentages not checked) — [CHI 96 proceedings](https://chi1996.acm.org/proceedings/papers/Page/srp_txt.html). [Moderate]

### Inferences
- Ship good defaults; let users save and share workspace layouts and keymaps as plain text files in the project (social spread, Mackay); offer presets ("Blender-like", "Unity-like" keymaps) to remove the "make new behave like old" barrier.
- Suggest customization at trigger moments (e.g. "you used this command 20 times from the menu; bind a key?").

### Gaps
- No recent large-scale data on customization rates in creative/pro tools.

## Accessibility in professional tools

### Takeaway
Accessibility in an editor is mostly the same as for any app: full keyboard operation, a real accessibility tree for screen readers, colour not used as the only signal, adequate contrast, and scalable UI. These help power users too (keyboard-first, high-DPI scaling).

### Cited Findings
- WCAG 2.2 (W3C Recommendation) criteria that map directly onto editors: 2.1.1 Keyboard (all functionality operable by keyboard), 2.4.7 Focus Visible, 1.4.1 Use of Color (colour not the only means), 1.4.3 Contrast minimum 4.5:1 for text, 1.4.11 Non-text Contrast 3:1 for UI components, 1.4.4 Resize Text up to 200% (not re-fetched; standard) — [WCAG 2.2](https://www.w3.org/TR/WCAG22/). [Standard, consensus]
- Colour vision deficiency affects roughly 1 in 12 men and 1 in 200 women (not re-fetched) — [Colour Blind Awareness](https://www.colourblindawareness.org/colour-blindness/). Red/green status colours (errors vs OK, gizmo axes) are the usual failure; pair colour with shape, label or pattern. [Strong epidemiology]
- Game Accessibility Guidelines provide a practitioner checklist (remappable controls, text size, colour-blind modes) — [gameaccessibilityguidelines.com](https://gameaccessibilityguidelines.com/). [Opinion/consensus]

### Inferences
- The engine already has an accessibility tree and an `accesskit` bridge in `sage_ui`; the editor should expose every panel, inspector field and block through it, with keyboard paths for block editing (move, insert, wrap) so blocks are not mouse-only.
- Provide a UI scale setting and a colour-blind-safe default palette for gizmos, diffs and error states.

### Gaps
- No empirical studies found this session on screen-reader use of game editors or block editors (there is work on accessible block languages, e.g. Microsoft's Code Jumper / Google's accessible Blockly, not retrieved).

## Implications for a game engine editor at each skill level

These are inferences from the findings above, applied to this engine (Python scripting, ECS, deterministic simulation, block editor as a view of Python, inspector, scene editor).

### Beginner (first hours)
- Start from a runnable template (Pong) and make the first action an edit to it (Carroll minimalism; paradox of the active user).
- Offer a user-chosen reduced layer (Shneiderman multi-layer; training wheels), not adaptive menus (Findlater & McGrenere). Show a way to see what is hidden.
- Direct manipulation in the scene and inspector; immediate feedback; blocks to avoid syntax errors.
- Contextual help on hover with short clips (ToolClips' 7x result).
- Undo for everything, including play-mode experiments via deterministic snapshots (exploration needs trust in undo).
- Accessibility from day one: keyboard paths, contrast, colour plus shape.

### Intermediate (most users, most of the time; Cooper)
- Make the transition costless: blocks show their Python; menus and palette results show hotkeys; right-click radial menus that can be flicked (Kurtenbach & Buxton rehearsal).
- Command palette with aliases, hotkeys and menu paths in results (Furnas vocabulary problem); include settings and help.
- Gentle nudges to faster methods: optional audio or a noticeable hint when a command with a hotkey is used via menu (Grossman 2007: +100% hotkey use); suggest binding a key at Mackay-style triggers.
- Vocabulary extension: "related commands" and usage-based recommendations (CommunityCommands).
- Inspector features that reveal ECS hidden dependencies (Cognitive Dimensions).

### Expert
- High ceiling: full Python, text view authoritative, scriptable editor commands, rebindable keys, saved/shareable workspaces and keymaps as text files in the project (Mackay, Banovic).
- Spatially stable layouts (CommandMaps), keyboard-first everything, palette as the primary entry point.
- Selective/branching history panel and deterministic replay to compare variants (Kurlander, Myers selective undo).
- Measure extended learnability (Grossman et al. 2009): opt-in local usage data, as the ribbon team did, to find where users plateau.
