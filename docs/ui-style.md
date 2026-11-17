# UI style

Rules for the text and look of the engine's own UI (the editor, the UI playground and demo). Set by Morgan on 2026-11-14.

## Capitalization
Do **not** write UI text in all capitals. It reads as shouting, is harder to scan, and is a common tell of generated designs.

| Where | Style | Examples |
|---|---|---|
| Buttons, tabs, headings, menu items, table and column titles, dialog titles | **Title Case.** Short words stay lower case: a, an, and, as, at, by, for, in, of, on, or, the, to, from. | `Undo`, `Add Component`, `New System`, `Suggest by Type`, `Derives From` |
| Hints, status lines, errors, tooltips, empty states, descriptions | **Sentence case.** A full stop only when there is more than one sentence or it reads as a sentence. | `Click to select. Drag to move.`, `Nothing selected`, `This slot wants a number` |
| Words on blocks in the block editor | **lower case**, the way the Python reads. | `set pos.x to`, `change`, `if`, `for each … in`, `else` |
| Names from the project (entities, components, fields, functions, files) | **As written.** Python is case sensitive. Never change them with `.upper()`. | `Position`, `pos.x`, `game.py` |
| Key names | Their usual capitalization. | `Ctrl+B`, `Alt+Up`, `Shift+F6`, `Esc` |

Acronyms stay capitalized (OK, UI, ID, RGB).

## Why this is not a font problem
The built-in 5x7 pixel face only has upper case, so games using it show upper case. That is a limit of the face. The editor uses Liberation Sans (see `docs/dependencies.md`), which has both cases; the rule above applies to it and to anything else drawn with a full face. Write the strings in the case they should be shown in, rather than relying on a font to change them.

## Tests
Tests that compare UI text should use the real casing, or compare case-insensitively when the case is not the point (the editor tests' `texts()` helper upper-cases what it returns).
