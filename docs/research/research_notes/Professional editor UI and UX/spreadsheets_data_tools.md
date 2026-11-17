# Spreadsheets, database-as-app tools and reactive notebooks: lessons for a game editor

Research date: 2026-11-14. Scope: why spreadsheets (Excel above all) are the most successful end-user programming environment, what database and notebook tools add, the known failure modes, and how this maps to an ECS editor where "a game is a small database that changes over time".

Source-quality note: Items marked "(not fetched this session)" are cited from well-known primary URLs that I did not re-open during this research pass. Verify them before quoting exact wording. Everything else was seen in search results or fetched pages during this session.

---

## 1. Why spreadsheets work (the theory)

### Takeaway
Spreadsheets give live, visible values in a grid that is both the data and the program. Users can start with no programming at all and add formulas one cell at a time (the "gentle slope"). Each cell is a small, purely functional program that recalculates on its own. People adopt this because the payoff is immediate and the attention they must invest is small at every step.

### Cited Findings
- **Low threshold, high ceiling.** Myers, Hudson and Pausch (ACM TOCHI 7(1):3–28, 2000) define "threshold" as how hard a system is to learn and "ceiling" as how much can be done with it. They say the most successful tools were either low threshold with a low ceiling or high threshold with a high ceiling, and that getting both a low threshold and a high ceiling is an important unsolved problem. High-threshold systems often went unadopted because users never got past the initial learning barrier. — [researchr record](https://researchr.org/publication/MyersHP00); summary in [UCSD lecture notes](https://d.ucsd.edu/IxD/research/2019/lectures/discussants/10-Tools.pdf). Note: the authors are Myers, **Hudson** and **Pausch**, not "Smith/Horn".
- **Gentle slope.** MacLean, Carter, Lövstrand and Moran, "User-tailorable systems: pressing the issues with buttons", CHI 1990, pp. 175–182, DOI 10.1145/97243.97271. They named the "customization gulf" (the skill jump once tailoring goes beyond setting parameters). They argued for a gentle slope of rising complexity in which each step needs only a small, incremental amount of new learning. Their paper has the well-known "steep vs gentle slopes of tailorability" diagram. — [researchr](https://researchr.org/publication/MacLeanCLM90); [Ludwig et al., practice-oriented EUD](https://researchgate.net/profile/Thomas-Ludwig-2/publication/319046652_A_Practice-Oriented_Paradigm_for_End-User_Development/links/5d4a7e16a6fdcc370a80f1a8/A-Practice-Oriented-Paradigm-for-End-User-Development.pdf); [pattern summary](https://pattern-playground.onrender.com/patterns/malleability). I found only secondary sources, not the full text.
- **Nardi & Miller.** "Twinkling lights and nested loops: distributed problem solving and spreadsheet development", *Int. J. Man-Machine Studies* 34:161–184, 1991 (HP Labs).
  - Spreadsheets split into two programming layers: values/layout, and formulas.
  - This split lets people with different programming skill and domain knowledge work together.
  - The spreadsheet becomes a shared "cognitive artifact" that mediates cooperative work.
  - The automatically updating values ("twinkling lights") keep users motivated.
  - Earlier version: CSCW 1990, an ethnographic study of distributed spreadsheet development.
  - Sources: [PDF mirror (lri.fr)](https://www.lri.fr/~mbl/Stanford/CS477/papers/Nardi-Twinkling-IJMMS.pdf); [miramontes.com copy](https://miramontes.com/writing/twinklinglights); [CSCW'90 paper](https://www.miramontes.com/writing/ethno-spreadsheet/index.php); [Nardi & Miller, "The spreadsheet interface: A basis for end-user programming"](https://www.miramontes.com/writing/spreadsheet-eup/).
- **Nardi's book.** *A Small Matter of Programming: Perspectives on End User Computing* (MIT Press, 1993) extends this into a general argument: end users program happily in task-specific languages (spreadsheets, CAD) when the language matches the domain and collaboration is supported ("local developers" and "gardeners" help others). — (not fetched this session) [MIT Press](https://mitpress.mit.edu/9780262140539/a-small-matter-of-programming/)
- **Attention investment (Blackwell; used by Burnett).**
  - Blackwell's model treats programming as a cost-benefit decision: up-front attention spent building an abstraction versus attention saved later, with risk, because the payoff is not guaranteed. — [Blackwell, HCC'02 "First steps in programming: a rationale for attention investment models"](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf)
  - Blackwell names Burnett's "Surprise–Explain–Reward" strategy as one way to change how users perceive this investment. — [Blackwell & Burnett-era work on end-user developers at home](https://cacm.acm.org/research/end-user-developers-at-home)
  - Peyton Jones, Blackwell and Burnett, "A user-centred approach to functions in Excel" (ICFP 2003), applies attention investment and Cognitive Dimensions to the design of user-defined functions in Excel. — [paper](https://typeset.io/pdf/a-user-centred-approach-to-functions-in-excel-2fpvo9q9kk.pdf)
- **Excel formulas are functional programming.**
  - Peyton Jones calls Excel "the world's most widely used functional programming language". Formulas are purely functional, with "no side effects".
  - He estimates "a hundred times as many users of formulae in Excel as there are professional programmers". This is an estimate, not a measured figure.
  - Most Excel users are end-user programmers who write formulas to get other work done.
  - Source: [MSR podcast with Gordon & Peyton Jones, 5 May 2021](https://www.microsoft.com/en-us/research/?p=743248)
- **VisiCalc's origin.** Dan Bricklin's own history pages (bricklin.com) describe the idea as an "electronic blackboard" with immediate recalculation, which came to him in a Harvard Business School class around 1978, and VisiCalc shipped for the Apple II in 1979 with Bob Frankston. — (the fetch failed with a TLS certificate error, so this is not verified this session) [bricklin.com/history/saiidea.htm](http://www.bricklin.com/history/saiidea.htm)

### Inferences
- The spreadsheet's three key properties are that every value is visible, every value is live, and every formula is small and local. Each one lowers the attention cost of the next step, which is the attention-investment explanation of the gentle slope.
- Nardi & Miller's two layers match the engine directly: the component values (the data layer) can be edited by designers, while systems and queries (the formula layer) are written by more technical collaborators. Both work on one shared artifact.
- Burnett's Surprise–Explain–Reward matches the planned "Why?" button. The surprise (a rule did not fire) prompts a cheap explanation, and the explanation teaches the next abstraction.

### Gaps
- I could not open Bricklin's primary pages (TLS error). The VisiCalc details above are from memory of his site and need checking.
- I did not open the full text of MacLean et al. 1990.
- Burnett's original Surprise–Explain–Reward papers (Wilson, Burnett et al., CHI 2003, "Harnessing curiosity to increase correctness in end-user programming") were not retrieved.

---

## 2. Excel's specific UX features

### Takeaway
Excel's power comes from layering. You can type a number. Then you can write a formula, picking cells by clicking and seeing the references colored. Then you can audit it with trace arrows and Evaluate Formula. Then you can name it (Names, Tables), spill it (dynamic arrays), abstract it (LET, LAMBDA), and finally drop into Python. Each layer is optional and lives in the same cell and formula bar.

### Cited Findings
- **Auditing tools** (Formulas tab > Formula Auditing group):
  - Trace Precedents shows what feeds a formula (backwards). Trace Dependents shows what a cell feeds (forwards). Arrows are blue, and red when an error is involved.
  - Evaluate Formula steps through a formula's evaluation one sub-expression at a time.
  - Background error checking puts a green triangle in a cell's top-left corner when a rule fires (inconsistent formula, omitted adjacent cells, number stored as text).
  - Recommended workflow: Error Checking to find the problem, Evaluate Formula to diagnose it, trace arrows to confirm the source.
  - Sources: [Journal of Accountancy, Jun 2024](https://www.journalofaccountancy.com/issues/2024/jun/how-to-find-errors-in-excel-formulas.html); [ICAEW "How to review a spreadsheet" part 5 (2022)](https://icaew.com/technical/technology/excel-community/excel-community-articles/2022/how-to-review-a-spreadsheet-part-5); [excel-easy](https://www.excel-easy.com/examples/formula-auditing.html). All are secondary; I did not retrieve Microsoft's own support page.
- **Dynamic arrays and spill.**
  - Microsoft's Excel blog previewed dynamic arrays on 25 Sep 2018, with FILTER, UNIQUE and SORT in the initial function set, for Insiders first. — [Excel blog, "Preview of Dynamic Arrays in Excel"](https://techcommunity.microsoft.com/blog/excelblog/preview-of-dynamic-arrays-in-excel/252944)
  - An update on that post (1 Jul 2020) says dynamic arrays are available to Office 365 users on all endpoints. — [same post](https://techcommunity.microsoft.com/blog/excelblog/preview-of-dynamic-arrays-in-excel/252944)
  - Not in Excel 2019 or earlier; available in Excel 365 and 2021. — [Exceljet](https://exceljet.net/dynamic-array-formulas-in-excel); [Microsoft support: spilled array behavior](https://support.microsoft.com/en-us/Excel/dynamic-array-formulas-and-spilled-array-behavior)
  - Before this, array formulas needed Ctrl+Shift+Enter and a size fixed in advance. With spill, "you don't have to decide in advance how big the array is going to be". — [MSR podcast 2021](https://www.microsoft.com/en-us/research/?p=743248)
- **LET and LAMBDA** (Microsoft Research Cambridge "Calc Intelligence" team with the Excel team).
  - LAMBDA was announced in Dec 2020. It lets users define functions in the formula language itself; named LAMBDAs can call each other, including recursively.
  - Gordon & Peyton Jones: "With LAMBDA, Excel has become Turing-complete." The work was presented around POPL 2021.
  - LET gives names to subexpressions within a formula.
  - Sources: [MSR podcast](https://www.microsoft.com/en-us/research/?p=743248); [The New Stack](https://thenewstack.io/microsoft-excel-becomes-a-programming-language/); [InfoQ](https://www.infoq.com/articles/excel-lambda-turing-complete); [TechRepublic](https://www.techrepublic.com/article/microsoft-turning-excel-into-a-turing-complete-programming-language/)
- **Data types (records in cells).** Cells used to hold only text or numbers. Linked "entities" added records, with a dot operator for fields (`A1.price`), and records can nest arrays of records. — [MSR podcast](https://www.microsoft.com/en-us/research/?p=743248)
- **References are first-class values.** A function can receive a range as a reference, not only its values. — [MSR podcast](https://www.microsoft.com/en-us/research/?p=743248)
- **Python in Excel.** Microsoft's message-center notices give these dates:
  - GA on Windows desktop (standard compute) from Sept 2024, after an Aug–Oct 2024 rollout window. — [MC833882 archive](https://app.cloudscout.one/evergreen-item/mc833882/)
  - Excel for the web: rollout from mid-Feb to late Mar 2025. — [MC960802](https://mc.merill.net/message/MC960802)
  - Eligibility extended to web-only commercial licenses (E1, Business Basic, F3) by Jun–Aug 2025. — [MC1042933](https://mc.merill.net/message/MC1042933)
  - The roadmap entry RM526786 lists Desktop/Mac/Web GA as "January CY2026" (last updated 18 Feb 2026). — [RM526786](https://mc.merill.net/message/RM526786)
  - The public preview was announced in Aug 2023. Python runs in Microsoft's cloud, not locally. — (the preview date is from memory; not fetched this session)
- **Ribbon redesign (Office 2007) and how it was evaluated.** Jensen Harris's "An Office User Interface Blog" (2005–2007), in the "Why the UI" series:
  - Design used opt-in telemetry from the Customer Experience Improvement Program (internally "SQM").
  - Fewer than 2% of Office 2003 users customized their UI, so the team focused on the default experience.
  - Resolution data showed 1280x1024 and up was common, which informed sizing.
  - Part 8 ("Grading on the Curve") warns against designing purely from usage counts.
  - Julie Larson-Green said commands on the first (Home) tab were chosen from CEIP data.
  - The "Real People Study" posts describe field studies with real users.
  - Sources: [Grading on the Curve (Part 8)](https://learn.microsoft.com/en-us/archive/blogs/jensenh/grading-on-the-curve-why-the-ui-part-8); [Inside Deep Thought (Part 6)](https://learn.microsoft.com/fr-fr/archive/blogs/jensenh/inside-deep-thought-why-the-ui-part-6); [Mythbusters](https://learn.microsoft.com/id-id/archive/blogs/jensenh/mythbusters-the-office-12-new-ui); [Real People Study, Part 3](https://learn.microsoft.com/en-us/archive/blogs/jensenh/putting-the-feedback-to-work-real-people-study-part-3); [Enter the Ribbon](https://learn.microsoft.com/en-nz/archive/blogs/jensenh/enter-the-ribbon); [Microsoft PressPass interview](https://news.microsoft.com/?p=10930)
  - "Paste is the most used command" comes only from a reader comment on Part 7, so treat it with caution. — [Part 7](https://learn.microsoft.com/es-es/archive/blogs/jensenh/no-distaste-for-paste-why-the-ui-part-7)

### Inferences
- Several Excel features map directly onto engine features:

  | Excel | Game editor equivalent |
  |---|---|
  | Point-and-click references plus colored highlighting | Clicking an entity or component while editing a query or block inserts a typed reference, and the referenced rows or cells light up in the world view |
  | Trace Precedents/Dependents | "Which systems read/write this component" and "which components does this system touch" — Flecs query terms make this static and exact |
  | Evaluate Formula | Step through a query's terms on a chosen entity, showing which term failed (this is the "Why?" button) |
  | Green-triangle error checking | Lint markers on rows: an entity missing a component that a likely system needs, or an inconsistent value in a column of otherwise similar entities |

- The LET/LAMBDA/Python sequence shows how to raise the ceiling without raising the threshold. Every new layer is reached from the same place (a cell or formula bar), and the beginner layer stays untouched.
- Spill is the closest Excel idea to an ECS query. One formula produces a variable-sized result range, much as a query yields a variable set of matching entities. Excel shows the spilled region with a border and gives a `#SPILL!` error when it is blocked. That is a useful model for showing query results inline.

### Gaps
- I did not retrieve Microsoft's own support pages on Trace Precedents, Evaluate Formula and error checking, nor documentation on reference color-coding (references colored in the formula bar and on the grid). These are well-known behaviors, but I have no primary citation for them.
- There is no primary source here for structured references (Excel Tables, `Table1[Price]`, introduced in Excel 2007) or for function autocomplete (Formula AutoComplete, Excel 2007).
- No usability study numbers for the ribbon were found, only the telemetry rationale.

---

## 3. Failure modes: spreadsheet errors, hidden state, no testing

### Takeaway
Spreadsheets fail quietly. About 1–5% of formula cells contain errors, so large sheets almost always contain some. Errors that matter in public come from ranges that miss rows, format limits that silently drop data, and logic hidden in cells nobody inspects. The fixes researchers propose are structure (tables, names), smell detection, review, and testing. Reactive notebooks show the same "hidden state" problem in another setting.

### Cited Findings
- **Panko (cell error rates).**
  - Field audits found 1.1%, 2.2% and 2.5% of cells with original errors. Lab experiments found whole-spreadsheet cell error rates of at least 1–2%. — [Panko, "Spreadsheet Errors: What We Know. What We Think We Can Do" (EuSpRIG 2000)](https://arxiv.org/pdf/0802.3457)
  - A later Panko paper pools 14 lab studies of spreadsheet development (967 participants working alone) at a 3.9% cell error rate. He says human error research predicts 1–5%, and argues that developers and managers resist believing these numbers. — [Panko, "What We Don't Know About Spreadsheet Errors Today" (2016)](https://arxiv.org/pdf/1602.02601)
  - Panko–Halverson error taxonomy (mechanical, logic, omission errors). — [Revisiting the Panko–Halverson taxonomy](https://arxiv.org/pdf/0809.3613)
- **Reinhart–Rogoff (2013).** Herndon, Ash and Pollin found a coding error in the Reinhart–Rogoff working spreadsheet. It excluded five countries (Australia, Austria, Belgium, Canada, Denmark), because the average covered rows 30–44 instead of 30–49. That error alone accounts for about −0.3 percentage points in the highest-debt category. The critique also attributed part of the gap to selective data exclusion and unconventional weighting, so the Excel error was only one of several problems. — [New Statesman, Apr 2013](https://www.newstatesman.com/business/economics/2013/04/statistic-cited-defend-austerity-partially-based-excel-error); [ThinkAdvisor](https://www.thinkadvisor.com/2013/04/16/oops-reinhart-rogoff-research-tripped-up-by-typo/); [Naked Capitalism](https://nakedcapitalism.com/2013/04/linchpin-pro-austerity-paper-rife-with-errors-recomputed-results-show-no-growth-hit-from-high-government-debt.html)
- **UK COVID cases lost (Oct 2020).**
  - Public Health England left 15,841 cases (25 Sep – 2 Oct 2020) out of daily figures and contact tracing.
  - Cause: a pipeline used the legacy XLS format, which has a 65,536-row limit. Each test produced several rows, so each template held only about 1,400 cases, and the overflow was silently dropped.
  - A Warwick paper estimated more than 1,500 possibly preventable deaths. This is a single-study estimate.
  - Sources: [Yahoo/BBC-syndicated explainer](https://www.yahoo.com/news/excel-error-led-16-000-110938424.html); [digit.fyi](https://digit.fyi/excel-error-blamed-for-16000-unreported-coronavirus-cases/); [deaths estimate](https://uk.news.yahoo.com/data-glitch-may-led-more-161147064.html)
- **Formula smells and refactoring (Hermans).**
  - Hermans, Pinzger and van Deursen adapted Fowler's code smells to formulas (ICSM 2012) and defined metrics to detect them automatically. On the EUSES corpus, smells were common and exposed real errors. — [ICSM 2012 PDF](https://research.tudelft.nl/files/177578593/Hermans2012-formulasmells.pdf)
  - The 2015 Empirical Software Engineering version adds interviews with the owners of 10 real spreadsheets. Owners showed little awareness of the risks of complex formulas and kept extending formulas until they were long and hard to follow. Simple smells could be refactored. — [Hermans 2015 PDF](https://research.tudelft.nl/files/177577679/Hermans2015-smells.pdf)
- **Regulatory angle.** EuSpRIG papers discuss Sarbanes–Oxley controls over spreadsheets, since spreadsheets are untested production software in finance. — [Sarbanes-Oxley: What About all the Spreadsheets?](https://arxiv.org/pdf/0804.0797)
- **Hidden state in notebooks.** Pimentel et al. (MSR 2019) studied about 1.4M Jupyter notebooks from GitHub. Only about 24% ran without errors, and about 4% reproduced their original results. Out-of-order execution and hidden state were major causes. — (figures from memory; not fetched this session) [Pimentel et al., "A Large-scale Study about Quality and Reproducibility of Jupyter Notebooks"](https://doi.org/10.1109/MSR.2019.00077). Reactive notebooks were built to remove this hidden state (see section 5).

### Inferences
- The engine already addresses three classic spreadsheet failures:
  - **Hidden state:** deterministic simulation, state hashing and rewind.
  - **No testing:** replay tests make every recording a regression test.
  - **Silent truncation:** typed components with schemas and migrations.
- The editor should make these protections visible so users trust them. For example, a replay diff can show the first tick and entity where two runs diverge (like `first_divergence`).
- The Reinhart–Rogoff error was a range that failed to grow with the data. ECS queries match by component, not by position, so they avoid this class of error by construction. Any editor feature that selects entities by manual list (a hand-picked group, or index ranges in a table view) brings the risk back. Prefer selections defined by queries or tags.
- Spreadsheet smells such as long formulas, many references and duplicated logic have ECS equivalents: systems with very long queries, entities that copy rather than inherit from a prefab, and "magic" components that many systems write. An editor lint could flag these, the way Excel's green triangles do.

### Gaps
- EuSpRIG's horror-story catalog was not retrieved. Many more cases exist (for example JPMorgan's "London Whale" VaR model, 2012), but none were verified this session.
- The figures for the gene-name autocorrect problem (Ziemann et al. 2016, about 20% of genomics papers with supplementary Excel files affected, and HGNC renaming genes in 2020) are from memory and were not verified.

---

## 4. Database-as-app tools (Airtable, Notion, Access, FileMaker, Retool)

### Takeaway
These tools keep the spreadsheet's direct manipulation but add what spreadsheets lack:
- typed columns (fields)
- records with identity
- linked records (relations), with lookup and rollup through them
- several views of the same table (grid, kanban, gallery, calendar), each with saved filters, sorts and groups

A filter in a view works as a query for non-programmers.

### Cited Findings
- **Airtable.**
  - Lookup, rollup and count fields require a linked-record field.
  - A lookup adds a column pulling a chosen field from the linked table. A rollup summarizes linked records with aggregate functions.
  - Sources: [Softr's Airtable glossary](https://www.softr.io/airtable/airtable-glossary) (third-party); [Airtable support (Spanish), linking records](https://support.airtable.com/docs/es/linking-to-one-many-or-a-subset-of-records)
- **Airtable views.** Grid, Kanban, Calendar, Gallery, plus Timeline/Gantt. The same records look like a calendar to editors, a kanban board to production, and a filtered grid to analysts. — [Softr, guide to Airtable views](https://v2.softr.io/blog/airtable-views); [O'Reilly, *Learning Airtable*, ch. 6 "View Types"](https://www.oreilly.com/library/view/learning-airtable/9781098133368/ch06.html)

### Inferences
- The mapping to an ECS is almost exact:
  - A table is an archetype or a query result set.
  - A typed field is a component member with reflection.
  - A linked record is a Flecs relationship (`ChildOf`, `IsA`, or custom pairs).
  - Lookup and rollup are joins across relationship targets (for example "the parent's Position", "count of children with Health < 10").
  - A view is a saved query plus a presentation:
    - grid: an entity spreadsheet
    - gallery: sprite thumbnails
    - kanban: entities grouped by an enum or state component
    - calendar or timeline: an event log over ticks
- Airtable separates schema editing (field types) from data entry, and lets views be shared with filters that cannot be changed. That suggests the editor could hand a beginner a curated view ("the enemies table, these 4 columns") while hiding the full schema.

### Gaps
- No primary sources were retrieved for Notion databases (relations, rollups, the 2023+ database views), Microsoft Access (query-by-example designer, forms and reports), FileMaker (layouts over tables, layout mode vs browse mode), or Retool (component-bound queries with `{{ }}` bindings). Search budget ran out. From general knowledge, each of these ideas is relevant:
  - Access's QBE grid is the classic visual query builder for non-programmers.
  - FileMaker's browse/layout mode split is a precedent for separating play from edit.
  - Retool binds UI widgets to query results reactively.

  These should be verified before they are cited.
- Airtable's own help pages on views were not retrieved (only third-party descriptions).

---

## 5. Reactive notebooks and liveness research (Observable, Jupyter, marimo, Pluto.jl; Bret Victor; Ink & Switch)

### Takeaway
Reactive notebooks bring the spreadsheet's automatic recalculation to general code. They find each cell's definitions and references by static analysis and run cells in dependency order, which removes hidden state. They pay for this with constraints: each name has one definition, and in-place mutation is not tracked. Ink & Switch's "gradual enrichment" (start informal, add structure and computation only when needed) is the modern statement of the gentle slope.

### Cited Findings
- **marimo.**
  - It statically analyzes each cell, without running it, to find references and definitions. These form a DAG, and running a cell marks its descendants to run.
  - Execution order follows dependencies, not position. Reactivity makes notebooks "more reproducible by eliminating hidden state".
  - Deleting a cell deletes its globals from memory.
  - Mutations (`my_list.append(42)`, `foo.bar = 10`) are not tracked, so the docs say to avoid mutating in one cell what another defines.
  - Each global must be defined by exactly one cell. Underscore-prefixed names are local to a cell.
  - A lazy mode marks dependents stale instead of running them, and cells can be disabled along with their dependents.
  - Source: [marimo docs, Reactivity](https://docs.marimo.io/guides/reactivity/)
- **Pluto.jl.**
  - It tracks dependencies through the syntax tree so outputs stay in sync with code: "You can't accidentally use an old value." — [Pluto docs, Reactivity](https://plutojl.org/en/docs/reactivity/)
  - It rejects a variable defined in two cells ("multiple definitions" error). — [Julia Discourse](https://discourse.julialang.org/t/delete-all-variables-pluto-notebook-julia/93245)
  - Whether deleting a cell removes its variable is inconsistent across forum reports. — [Discourse thread](https://discourse.julialang.org/t/enum-persistence-in-pluto-jl/43703)
  - Architecture overview: [Pluto docs](https://plutojl.org/en/docs/architecture/)
- **Observable.** Cells are reactive values evaluated in topological order, with each value shown above its code. `viewof` binds an input widget to a value, and generators produce values that change over time. — (fetch failed with HTTP 429; from knowledge, not verified this session) [How Observable Runs](https://observablehq.com/@observablehq/how-observable-runs)
- **Ink & Switch, Potluck.** Users record information in messy text. Live searches extract structured data, formulas compute over it, and results appear back in the text as dynamic annotations ("gradual enrichment"). It worked for personal tools such as recipes, workouts and chores. — [Potluck essay](https://inkandswitch.com/potluck); [project page](https://www.inkandswitch.com/project/potluck/); ["Formality on demand"](https://www.inkandswitch.com/ink/notes/formality-on-demand/); Litt's [MIT PhD thesis (2023)](https://groups.csail.mit.edu/sdg/pubs/2023/litt_phd_thesis.pdf)
- **Ink & Switch, malleable software (2025).** The essay discusses Potluck, Patchwork and Embark as prototypes. — [Malleable software](https://www.inkandswitch.com/malleable-software/); [Simon Willison's summary, 11 Jun 2025](https://simonwillison.net/b/8743)
- **Ink & Switch, end-user programming essay** (introduces end-user programming). — [quoted via SilverBullet](https://silverbullet.md/End-User%20Programming)
- **Bret Victor.** "Learnable Programming" (2012) argues that programming environments should show the data, show state over time, let users read the vocabulary in context, and let them "create by reacting" and "create by abstracting" (start concrete, then generalize). "Inventing on Principle" (2012 talk) argues creators need an immediate connection to what they create, and shows a game editor with time scrubbing and trajectory traces. — (not fetched this session) [worrydream.com/LearnableProgramming](http://worrydream.com/LearnableProgramming/)

### Inferences
- An ECS already has the static dependency graph that marimo computes. Each system declares its query terms (reads and writes) and its pipeline phase. A "dependency graph" view (components ↔ systems), with live counts of matched entities, is the ECS equivalent of marimo's DAG.
- marimo's "one definer per name" rule has an ECS counterpart: flag components written by more than one system in the same phase, since these are a source of order-dependent bugs. marimo's limit on tracking mutation is the same problem as Python scripts holding state outside components. The engine's rule "keep mutable state in components" is exactly what makes reactivity and rewind sound, and the editor should enforce it visibly.
- Notebook "staleness" corresponds to editing a system during play. On live reload the editor can mark dependent views stale, or re-run from a snapshot, as Pluto does.
- Bret Victor's time-scrubbing demo is already possible with deterministic rewind. A column view of a component over ticks (a sparkline per entity) is "show state over time".
- Potluck's gradual enrichment suggests letting beginners start with untyped notes or tags on entities and promote them to typed components later.

### Gaps
- No verified primary page for Observable's runtime (rate-limited).
- Jupyter's own position on hidden state, and newer Jupyter reactive extensions, were not retrieved.
- Bret Victor sources were not re-opened. Ink & Switch's local-first essay (Kleppmann et al., Onward! 2019) was not retrieved; it is relevant to collaborative editing of a scene database.

---

## 6. Implications for a game engine editor at each skill level (beginner, intermediate, expert)

### Takeaway
Present the world as a live spreadsheet of entities, and let each skill level go one layer deeper on the same artifact. Beginners edit values and see the game react. Intermediate users filter, link and build queries visually. Experts write systems in Python and use tracing, explanation and replay diffs, much as Excel experts use LAMBDA, Python and the auditing tools. Never introduce a sharp skill jump, and never hide state.

### Cited Findings
- The success criterion is a low threshold with a high ceiling, reached through incremental steps with no "cliffs". — [Myers, Hudson & Pausch 2000](https://researchr.org/publication/MyersHP00); [MacLean et al. 1990](https://researchr.org/publication/MacLeanCLM90)
- Users invest attention only when the expected payoff is clear. Surprise–Explain–Reward is the lever. — [Blackwell HCC'02](https://www.cl.cam.ac.uk/~afb21/publications/HCC02a.pdf)
- Two collaborating layers (data and formulas) let novices and local experts share one artifact. — [Nardi & Miller 1991](https://www.lri.fr/~mbl/Stanford/CS477/papers/Nardi-Twinkling-IJMMS.pdf)
- Errors are inevitable at 1–5% of cells, so tools must make checking cheap. — [Panko 2016](https://arxiv.org/pdf/1602.02601)
- Reactivity removes hidden state but needs single-definition and no-untracked-mutation rules. — [marimo](https://docs.marimo.io/guides/reactivity/)

### Inferences

**Beginner (designer, child, first-time user).** Spreadsheet layer: values and direct manipulation.
- **Entity table view.** Rows are entities (the StableId shown as a row number), columns are components or fields, and edits apply live while paused or playing ("twinkling lights").
- **Selection sync.** Selecting a row selects the sprite in the game view, and the reverse, like Excel's formula bar showing the selected cell.
- **Curated views.** Kits or teachers supply views like Airtable's ("Enemies: name, speed, health") with the schema hidden.
- **Gallery and kanban views.** Sprites as cards; group by a state enum such as `GameState` or a custom `AIState`.
- **Simple filters.** A filter is a chip ("has Collider", "health < 10") that compiles to a query term.
- **Undo, rewind and scrub.** These are always available, so mistakes are cheap, which lowers the attention cost.
- **Inline lint.** Green-triangle style markers, such as "this entity has Velocity but no Position, so the Move rule won't run". This is the passive form of the "Why?" button.

**Intermediate (hobbyist, technical designer).** Names, tables, links, query builder.
- **Visual query builder.** Like Access's QBE grid or Airtable filters, its result spills as a live table, Excel-dynamic-array style. The matched count updates per tick, and the generated Python or Flecs query text is shown next to it so the builder teaches the language (blocks as a view of Python).
- **Click-to-reference.** While editing a block or expression, clicking a column or entity inserts a typed reference, colored like Excel's reference highlighting. Referenced cells light up in the table.
- **Relationship columns.** These work like linked records: parent, `IsA` prefab, custom pairs. They come with lookup and rollup columns ("parent.Position.x", "count(children with Enemy)").
- **Prefab inheritance shown like formulas.** An inherited value appears styled as "computed from prefab", and an override as a "literal" value. Editing the prefab updates instances, as `OnInstantiate Inherit` already does for UI style groups.
- **The "Why?" button as Evaluate Formula.** Pick an entity and a system, then step through the query terms and see which one failed (Burnett's Surprise–Explain).
- **Trace Precedents/Dependents for systems.** Selecting a component column highlights every system that reads or writes it, and the reverse.

**Expert (programmer, tool builder).** LAMBDA/Python layer: abstraction, testing, performance.
- **Python systems edited in place.** Live reload marks affected views stale, notebook style. There is a dependency graph of systems ↔ components ↔ phases (the marimo DAG equivalent), with warnings for components written by more than one system per phase (marimo's single-definer rule).
- **Time-series column views.** A component over ticks (sparklines), using deterministic rewind (Bret Victor's "show state over time").
- **Replay and regression tooling as the answer to spreadsheet untestability (Panko, Hermans).**
  - Recordings become tests.
  - `first_divergence` is shown as "first tick/entity/column that differs".
  - The state hash is shown per tick.
- **Smell lints** (Hermans-style) for systems: overlong queries, duplicated logic across systems, entities that copy prefab values instead of inheriting.
- **Escape hatches stay in the same surfaces.** Python sits behind blocks, as Python in Excel sits behind cells. Saved queries can be named and reused, like LET/LAMBDA.

**Cross-cutting rules drawn from the failure literature**
- Selections should be query-defined, not hand-picked or index-ranged. This avoids Reinhart–Rogoff-style ranges that fail to grow with the data.
- Never truncate silently (the PHE/XLS lesson). Limits in views, scene files or saves must fail loudly.
- All state lives in components that are visible in the table. No Python-variable state, which mirrors marimo's mutation caveat and the engine's own rule.

### Gaps
- No empirical study was found of spreadsheet-style entity editors in game engines (for example Unity's or Godot's table or inspector plugins, or Flecs Explorer) or of how users of different skill levels use them. That would need a separate search.
- No controlled evidence was found that query builders help non-programmers learn the textual query language (the "blocks teach Python" hypothesis). This is a hypothesis to test with users.
