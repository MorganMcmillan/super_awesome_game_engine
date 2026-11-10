# Design exercises (for Morgan)

Last updated: 2026-11-09 · Companion to `claude/game-engine-design-doc.md`

These are **design homework**, not engineering tasks. Claude Code should not attempt them. Engineering tasks (Phase 0 spikes, benchmarks, tests) are in sections 13.4 and 13.5 of the design doc.

Numbers 1–14 match the references in the design doc ("exercise 8" and so on). Exercises 15–17 are new and came from the latest conversation. Tick the box when an exercise is done, and record the answer in the design doc by changing the relevant tag and adding a dated note.

## Suggested order

1. **Start with the four that shape Milestone 0:** 16, 8, 10, 13.
2. Then the data-model exercises: 12, 15, 6, 7.
3. Then language and blocks: 1–5 (these matter once the editor work starts, M3–M4).
4. Then UI and gameplay: 11, 14, 9.
5. Do 17 whenever you want a break from the heavier ones.

## Shapes Milestone 0

### [ ] 16. Milestone 0's third assertion
**Task.** Milestone 0 has two acceptance assertions already: (A1) replaying the same recorded input twice gives the same world-state hash at every tick, and (A2) a scripted headless Pong run reaches `GameOver` with the expected winner and score. Write the third (A3) so that a script can check it.
**Candidates to start from.** The contact table's begin/stay/end invariants (for example, a bounce fires exactly once per contact); restoring a snapshot at tick N reproduces the hash recorded at tick N; a Python rule cannot change the outcome between two runs.
**Feeds.** Design doc 13.1.
**Done when.** The assertion is one sentence, a script could check it, and you can say what specific thing is broken if it fails.

### [ ] 8. Collision schema, trace and begin vs stay
**Task.** Write `Contact(tick, a, b, normal, depth)` and the three derived views (begin, stay, end) as SQL. Trace this scenario: the ball touches the paddle for three consecutive ticks, and on the third tick it also touches the top wall. List the rows in `Contact` at each tick and which rules fire. Then answer: should "ball hits paddle" fire on begin only, or on stay too? What goes wrong in each case?
**Feeds.** 8.2, and the Milestone 0 collision system.
**Done when.** You have the SQL, a per-tick trace table, and a decision with the failure you would see under the other choice.

### [ ] 10. Single-writer table
**Task.** Build a table with fields as rows (`Position`, `Velocity`, collider shape, contact events) and phases as columns (script, pre-step sync, physics step, post-step sync). Each cell names the one system allowed to write that field. Fill it in twice, once for a kinematic character and once for a dynamic crate. Any cell with two writers is a race.
**Feeds.** 8.3.
**Done when.** Both tables are filled in, the first cell with two writers is found, and you have proposed a fix.

### [ ] 13. Host interface contract
**Task.** List everything the simulation needs from presentation, and everything presentation needs from the simulation. Check that nothing flows back from presentation into the simulation.
**Feeds.** 2 (principle 9), 3.2, the `Renderer` trait in Milestone 0.
**Done when.** The two lists are complete and you can state the narrowest interface that satisfies both.

## Data model

### [ ] 12. Component schema catalog
**Task.** Design the catalog every consumer reads: an `information_schema` for components and fields with columns such as type, default, range, docs, module, `since_version` and `renamed_from`. Show how renaming `Health.hp` to `Health.current` flows through it into a migration, a Python class, a block and a scene file.
**Feeds.** 4.7, 10.4.
**Done when.** You have the table definitions and a worked rename that touches all four outputs.

### [ ] 15. Core vocabulary
**Task.** Treat the core vocabulary as the target table that other modules' foreign keys point to. List the components it must contain so that the four games' modules (platformer, Pokémon-like, survival, RPG) never reference each other directly. Any component you can't place there marks a hidden coupling between two modules.
**Feeds.** 5.3, 7, Appendix B.2.
**Done when.** You have the component list, a note on which games use each, and a leftover list of couplings.

### [ ] 6. Relational model of the engine
**Task.** Model components, fields, entities, `IsA`, has, systems and system terms. Write queries for "which systems run on `goblin_17`?" and "how many entities are affected if `Frozen` is added to the Goblin prefab?" (a recursive CTE handles `IsA`).
**Feeds.** 5.1, 5.4, the blast-radius count in 5.2.
**Done when.** The schema and both queries run mentally against a small example you wrote out.

### [ ] 7. Limb schema and the arm swap
**Task.** Design `Rig`, `Slot`, `Limb` and `Occupies`, and the transaction that swaps an arm. Which constraints must hold (limb fits slot, one limb per slot, animation channels covered)?
**Feeds.** 6.
**Done when.** You have the schema, the constraints, and the transaction with its failure cases.

## Language and blocks

### [ ] 1. Breakout as a diff from Pong
**Task.** Build Breakout as a diff from the refactored Pong (Appendix A). Count identical, changed and new rules. Name the reusable ball/paddle/score kit core.
**Feeds.** 7 (the kit test: the diff between games should be small).
**Done when.** You have the three counts and a named core.

### [ ] 2. Pong through `ast`
**Task.** Run the refactored Pong, written as decorated Python, through `ast`. Make a table of distinct node types mapped to block shapes (statement, reporter, predicate, hat, type). Pick the three most exotic and decide: lock the function or add a new block.
**Feeds.** 4.1, 4.2.
**Done when.** The table exists and each exotic case has a decision.

### [ ] 3. Literal conversion matrix
**Task.** Build the conversion matrix for literal types (always safe, safe if it parses, refused). Pick the three most confusing cells and write the UI message for each.
**Feeds.** 4.4.
**Done when.** The matrix is complete and the three messages are written.

### [ ] 4. Command, reporter or predicate?
**Task.** Classify `list.append`, `list.pop`, `list.sort`, `sorted`, `dict.get`, `dict.setdefault`, `str.upper` and `world.spawn(Bullet)` as command, reporter, predicate or ambiguous. For each ambiguous one, decide: alias or `discard`.
**Feeds.** 4.5.
**Done when.** Every function has a classification and every ambiguous one has a decision.

### [ ] 5. Socket assignability matrix
**Task.** Build the matrix for sockets over `int`, `float`, `bool`, `str`, `None`, `[int or None]` and `any`. Decide the beginner-facing message for each refusal.
**Feeds.** 4.6.
**Done when.** The matrix is complete and each refusal has a message.

## UI, phases and gameplay

### [ ] 11. Normalize the UI style schema
**Task.** Group about 15 CSS-like properties into components using co-read and co-write reasoning. Mark each group inherited or not, and authored or computed. Test your design with two cases: a dropdown whose hover changes background and text color, and a parent whose font size changes (and should affect every descendant). List the systems, their order and who writes each field.
**Feeds.** 9.2.
**Done when.** You have the grouping, the flags, and both test cases walked through.

### [ ] 14. Pokémon-like damage resolution order
**Task.** Define the order in which damage is resolved for the Pokémon-like game, including allies that can hit your own creatures and elemental effects.
**Feeds.** 11.
**Done when.** You have an ordered list of steps, with a worked example including an ally hitting a friendly creature.

### [ ] 9. Phases: tables, run order and the insert transaction
**Task.** Design tables for `phase`, `system` and `module`. Write a recursive CTE that returns the run order of phases and detects cycles. Write the transaction for "insert phase Particles between Resolve and React", and show what the data looks like if only one of its two edits runs.
**Resolved part.** The earlier question of whether disabling a module silences its observers is answered: a Flecs maintainer confirmed on Discord (reported by you) that it does. Claude Code covers it with a regression test (spike S2), so the experiment is not part of this exercise any more.
**Feeds.** 5.5.
**Done when.** You have the tables, the CTE, and the transaction with the half-applied state.

## Naming

### [ ] 17. Name decision matrix and the first 30 seconds
**Task.** Build a decision matrix with criteria as rows and your top three names as columns. Suggested criteria: the say-it-aloud test ("I made this in ___"), searchability, kid-friendliness, mapping to features, mascot potential, and availability (domain, crate, PyPI, GitHub, itch.io, trademark). Weight the rows and score each name. Then sketch what a child sees in the first 30 seconds of a new project when the mascot is the default sprite.
**Candidates so far.** Horsepower, Gallop, Paddock, Stable, Rowdy, Beanstalk, Trellis, Axolotl. A horse-themed family would be Gallop (engine), Paddock (editor), Stable (kit library), with Horsepower as a tagline.
**Feeds.** Design doc 15.
**Done when.** The matrix produces a winner, or a tie-break question you can answer after checking availability.
