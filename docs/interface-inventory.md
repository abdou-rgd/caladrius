# Interface inventory

Written for T-131 (iteration T-130, wave 1) on 2026-11-06, from `main` at commit `0748ccb`. It lists what a
new interface would have to port: every screen and panel of `crates/caladrius-ui`, what `apps/caladrius`
and `apps/caladrius-web` do around it, the commands each screen calls, the state it keeps, the numbers it
formats, its tests and snapshots, and the cost of a port. Nothing here is a plan to change the code.

Inputs of later cards: T-133 reads the section **Plots**; T-134 reads **Engine access outside
`execute`** and **Platform code**; T-135 to T-137 read the screen sections, NCA first.

## 0. How to read this file

- **Lines** are `wc -l` of the file, inline unit tests included. The 35 source files of
  `crates/caladrius-ui/src` that are not test files total **15,360 lines**; the nine `tests*.rs` files add
  4,344; `examples/snapshot.rs` has 736 and `src/theme.json` 69. Every one of the 35 files is in exactly
  one section below (a shared file is listed once, in section 1).
- **Command ids** are written as in the registry, `COMMANDS` in `crates/caladrius-engine/src/commands/`.
  The registry has 25 commands. Settings are not commands: a setting has a name in the `Settings`
  structure and is changed by an `Action`, never by `execute`.
- **Serialised** means a field of `UiState` or `Settings` that is `Serialize` without `#[serde(skip)]`
  (golden rule 7: the state of the interface is data). Be careful with the word: the two apps persist only
  `Settings` (a file on the desktop, the local storage of the browser). `UiState` round-trips in tests but is
  never written by an app, and a project file holds only the engine `Project`. "Not serialised" means
  `#[serde(skip)]`, a field of `UiApp` outside `UiState`, or egui temporary memory.
- **Port cost** is in plain words, relative to the NCA screen of T-135 to T-137. "The engine does it" means
  the screen only lays out numbers the engine returned.
- **UX ids** are the headings of `specs/ux.md`. Only four ids are cited in the code comments (UX-FR-01,
  UX-FR-14, UX-SET-01, UX-LOG-02); the other mappings below are read from the text of the spec and the
  behaviour of the screen.
- **Tests** count `#[test]` functions. All the egui_kittest harnesses are built by one helper,
  `tests.rs::harness(app)` (1400 x 1000 points, wgpu). Most tests drive `UiApp` through `Action`s and check
  state; only some draw. "Harness k" below means k tests that draw a frame.

## 1. Whole crate at a glance

| Section | Files (lines) | Lines |
|---|---|---|
| 2. Shell, top bar, status bar, welcome, and shared files | `app.rs` 1253, `lib.rs` 82, `fmt.rs` 297, `theme.rs` 444 (+ `theme.json` 69), `widgets.rs` 133, `model.rs` 598, `logo.rs` 103 | 2,910 |
| 3. Project tree | in `app.rs` (lines 1033 to 1135), no file of its own | 0 |
| 4. Project files, File menu, unsaved-changes dialog | `projectfile.rs` 310, `projectmenu.rs` 113 | 423 |
| 5. Data entry and paste | `entry.rs` 1144, `entryflow.rs` 142, `entrygrid.rs` 121, `entrywide.rs` 484, `unitpick.rs` 136 | 2,027 |
| 6. Import | `import.rs` 405 | 405 |
| 7. Worksheet | `sheet.rs` 419 | 419 |
| 8. Raw plot | `rawplot.rs` 779 | 779 |
| 9. Plot panel (drawing shared by NCA, fit, simulation) | `plot.rs` 496, `plotdata.rs` 294 | 790 |
| 10. Plot pages | `plotpage.rs` 796 | 796 |
| 11. NCA | `nca.rs` 1132 | 1,132 |
| 12. Model choice | `modelpick.rs` 210, `modelinfo.rs` 774 | 984 |
| 13. Fit | `fit.rs` 693, `fitform.rs` 466, `fitplots.rs` 184, `fitpreset.rs` 188, `fitresult.rs` 396, `flow.rs` 306 | 2,233 |
| 14. Simulation | `sim.rs` 506 | 506 |
| 15. Settings | `settings.rs` 634, `settingspage.rs` 225, `settingsflow.rs` 137 | 996 |
| 16. Verify a project | `verify.rs` 408 | 408 |
| 17. Command palette | `palette.rs` 552 | 552 |
| **Total** | 35 files | **15,360** |

The screens that a person sees, in `Selection` order: Welcome, Import, Entry, Worksheet, Analysis (NCA, fit,
simulation or plot, told apart by `Shown`), NewFit, NewSimulation, Settings, Verification. The palette, the
unsaved-changes question and the notice line are overlays.

### Commands called, by screen

| Command id | Called by |
|---|---|
| `data.preview` | import, data entry, worksheet edits (`app.rs` 483, 673; `entryflow.rs` 93) |
| `data.import` | import and data entry (`app.rs` 738; `entryflow.rs` 114) |
| `data.describe` | worksheet refresh (`app.rs` `refresh_sheet`, via `engine.execute`) |
| `data.set_cell` | worksheet cell edit (`app.rs` 632) |
| `data.set_column` | worksheet unit and column edit (`app.rs` 667) |
| `units.check` | data entry and worksheet unit cells (`entryflow.rs` 15, 42) |
| `nca.run` | NCA page (`app.rs` 547) |
| `analysis.get` | opening a stored analysis (`app.rs` 502) |
| `analysis.run` | "Run again" of any analysis (`app.rs` 620) |
| `fit.initial_estimates` | fit page (`flow.rs` 127) |
| `fit.evaluate` | fit page, live objective (`flow.rs` 165) |
| `fit.run` | fit page (`flow.rs` 193) and the preset reader (`fitpreset.rs` 80, on a scratch engine) |
| `model.simulate` | simulation page and the live curve of the fit page (`flow.rs` 182, 258, 290) |
| `export.table` | worksheet refresh (`app.rs` 443 to 448) |
| `plot.xy` | plot pages (`plotpage.rs` 706) |
| `export.plot` | plot pages, the SVG export (`plotpage.rs` 775) |
| `project.describe` | tree and palette (`app.rs` 437; `palette.rs` 388) |
| `project.save` | project file (`projectfile.rs` 188) |
| `project.new` | project file (`projectfile.rs` 291) |
| `project.load` | project file, through `Engine::load_bytes` (`projectfile.rs` 251) |
| `project.replay` | Verify a project, through `replay_bytes` (`verify.rs` 42) |

That is 21 commands. **The four registry commands that no screen calls:** `history.list`, `analysis.remove`,
`data.remove`, `analysis.compare`. The palette lists them (it lists all 25, read from `describe()`), and
picking one shows a notice that says it has no page yet and to use `caladrius-cli` or the MCP server.
There is therefore no way to delete an analysis or a worksheet, nor to compare two analyses, from the
screen today.

### Engine access outside `execute`

For T-134. The screens do not go through one door. `UiApp::call` runs a command and shows an error in the
status bar; `UiApp::ask` runs one quietly (the fit and simulation pages show the sentence on the page). Besides
those, the crate calls the engine as a Rust library in these places:

- `Engine::load_bytes` for `project.load` (`projectfile.rs` 251) and `replay_bytes` on a scratch
  `Engine::new()` for `project.replay`, with the typed report `replay::{Report, Status, Verdict, Step,
  Counts, EngineId, StateCheck}` (`verify.rs` 42 to 43). Neither is an `execute` call in the UI.
- `import_csv` on a scratch engine (`fitpreset.rs` 75): the fit presets are read by running `fit.run` with
  `{"preset": id}` on a scratch engine and echoing the options it reports.
- `engine.project()` (`app.rs` 841; `projectfile.rs` 76, 83, 102, 203), `engine.revision()`
  (`projectfile.rs` 70, used to know whether the project is modified without comparing it each frame) and
  `engine.history()` (`app.rs` 429).
- `caladrius_engine::describe()` (`palette.rs` 79, `fitpreset.rs` 56, `modelinfo.rs` 676) for the palette
  entries, the preset schema and the equations' parameter names.
- `caladrius_engine::project::FitView::ALL` (`fitform.rs` 167) and `seal::{stamp_line, svg_with_stamp,
  short}` (`plotpage.rs` 788, `verify.rs`).
- Seals are not in the `Value` that `execute` returns: they sit in `engine.history().entries()`. A bridge
  that returns a "sealed answer" has to decide where the seal comes from.
- The UI holds the `Engine` itself (`UiApp.engine`). A web or Tauri front would move it behind the bridge.

Every one of these needs a bridge call or a new command in a port; the first four are the awkward ones,
because they pass typed Rust values, not JSON.

### What is hard-coded in the interface that is not interface

- `modelinfo.rs`: the table of 12 models with their equations and the meaning of each parameter (about 770
  lines), and the painter code of the compartment diagram.
- `entry.rs` and `entrywide.rs`: paste parsing (quotes, tab or comma, wide-table detection), 1,600 lines
  that decide what text means before any engine command sees it. The UI computes no number there but does
  parse.
- `nca.rs` 957: the λz line is drawn from `(intercept - lambda_z * t).exp()` over 60 steps, with the
  standard `exp`, in the UI. It is the only curve the UI computes itself; the engine does not return it.
- `fmt.rs`: number display (section 2). It uses the standard `log10` and `floor`, not the `libm` the engine
  uses (the UI crate opts out of `libm`), so the last digit of a display could in principle differ between
  platforms. No test covers this.

## 2. Shell, top bar, status bar, welcome, and the shared files

**Files:** `app.rs` 1253, `lib.rs` 82, `fmt.rs` 297, `theme.rs` 444 (+ `theme.json` 69), `widgets.rs` 133,
`model.rs` 598, `logo.rs` 103. Total 2,910.

**What it draws.** `UiApp::ui` (`app.rs` 761 to 846) lays out one frame: top bar (`top_bar`, line 928: small logo,
name, File menu, "Open CSV...", "Type data", the "New analysis" menu with NCA, Model fit, Model simulation and
XY plot, a "Commands" button, and a light or dark theme button on the right), bottom status bar (the notice
or "Ready", and the version), left project tree, right plot panel (hidden for plot pages and for screens
without a plot), central scroll area with the selected screen, then the palette overlay and the
unsaved-changes dialog. The welcome page (`app.rs` 1216 to 1253) shows the large logo, the version, one
paragraph of help, and two buttons: "Open a CSV file..." and "Type or paste data...". "Verify a project" and
"Open a project" are only in the File menu (and the palette). Dropped files are handled in `UiApp::ui` (`app.rs` 778), and again in both apps (section 19).

**Shared files, listed here once:**
- `theme.rs` and `theme.json`: the design tokens (light and dark: colours, radii, spacing, plot widths and
  marker sizes) loaded from the JSON, and the egui style built from them. A contract test fails the build
  if a colour, radius or margin literal appears outside the theme.
- `fmt.rs`: the number display, described below.
- `widgets.rs`: combo box that reports the new choice, switchable threshold, titled section, units line,
  error box (uses `fmt::parse_number`, `fmt::exact`).
- `model.rs`: typed views of the engine's JSON answers (worksheets, analyses, overview), tolerant of extra
  fields. It computes nothing (uses `fmt::plain`).
- `logo.rs`: two PNG renderings from `assets/icons/` (256 px and 64 px) embedded with `include_bytes!`,
  uploaded as textures on the first frame, and `ICON_256` for the window icon.
- `lib.rs`: module list, `VERSION`, re-exports `Action, Request, Selection, UiApp, UiState, ThemeMode,
  Tokens`.

**Commands:** `project.describe` (the overview, `app.rs` 437), `data.describe` and `export.table`
(`refresh_sheet`, `app.rs` 443 to 448). All other commands are called by the screens in their own sections.

**State.**
- `UiState` (serialisable, but not saved by any app): `mode` (light or dark), `selection`, `log_axis`,
  `nca`, `fit`, `sim`, `palette`, `settings_query`, `entry`, `plot`. Each page struct is in its own
  section.
- `state.log_axis` is **one flag shared** by the NCA, fit and simulation plot panels, not one per plot.
- Not serialised, fields of `UiApp`: `engine`, `tokens`, `overview`, `sheet`, `units` (the list of units the
  engine recognises), `rejected`, `pending` (the import in progress), `notice`, `requests` (what the
  program must do: pick a file, save, close), `file`, `verification`, `commands` (the registry),
  `settings` (the `Settings` structure is serialisable and is the only thing the apps store),
  `system_theme`, the logo textures, `can_quit`.
- The program talks to the UI by `Action` (inward: ~40 variants) and `Request` (outward: pick a CSV, open or
  verify a project, save, close, save settings, save a file). That pair is the cleanest seam in the crate.

**Numbers formatted.** `fmt.rs` is the one place numbers become text. Its public functions:
- `number`: 4 significant digits by default, scientific outside 1e-3 to 1e5, decimal mark and digit count
  taken from `Display`.
- `fit_quality`: up to 5 decimals (R-squared and similar).
- `exact` and `exact_point`: the shortest text that parses back to the same value (editable fields).
- `parse_number`: accepts the decimal comma and the typographic minus.
- `parameter_label`, `with_labels`, `parameter_unit`: engine parameter names to display labels and units.
- `plain`, `words`: strip the engine's markup (backticks), turn a code into words.
- `set_display` / `display`: the display setting (`digits`, `comma`) is a `thread_local`, set once per frame
  by `settingsflow.rs`. A port that formats elsewhere must pass the display setting explicitly.

**Tests and snapshots.** `tests.rs` (1311 lines, 48 tests, 22 harness calls) is the shared file: import, NCA, fit,
simulation, flag and error tests, plus the contract tests (no colour, radius or margin outside the theme; sizes
and spacing are tokens; the UI code has no file or clock access; the app asks for a file picker instead of
opening files; every screen draws without a panic in both themes and both axes; the state of the UI
round-trips as data; the version is shown on the welcome page and in the status bar). Inline unit tests in
`fmt.rs` (from line 210), `theme.rs` (347), `logo.rs` (78), `model.rs` (514). Snapshots: 01, 48 (welcome,
light and dark).

**UX ids.** UX-FR-01 (the "New analysis" button, always visible), UX-FR-13 (theme tokens), UX-REQ-03 (errors
say what to fix, via the notice line and the error box).

**Port cost.** The layout is easy in any stack. The cost is in the three seams the shell owns: the
`Action`/`Request` pair (it maps well to a message bus in any stack), `fmt` (see section 1: the thread-local
display and the standard-library maths), and the tokens (`theme.json` already is data and can feed CSS
directly, which is why T-132 makes it the single token file). Roughly 2,900 lines, but a third of it is
tokens and typed views that translate mechanically.

## 3. Project tree

**Files:** none of its own. `app.rs` 1033 to 1135 (`fn tree`), fed by `model.rs`.

**What it draws.** The left panel: the project name with a `*` when modified and its file, a line "N analysis
result(s) out of date" when any is stale, the list of worksheets (name, rows by columns), then the list
of analyses (NCA, fits, simulations, plots), each named from its content ("NCA, oral 100 mg, subjects 1 to
12"). A stale analysis is drawn in the stale colour, says "out of date" and has a small "Run again" button
(`analysis.run`). A click selects. The stale marker is the interface's only display of the dependency
graph; the tree is flat, not nested by worksheet.

**Commands:** `project.describe` (the overview it reads); the "Run again" button runs `analysis.run`. **State:** `state.selection` (serialisable);
the overview is a cache.

**Numbers:** none (names come from the engine). **Tests:** `tests_project.rs` has the tree tests
(`analyses_are_named_from_their_content...`, `an_input_change_marks_every_dependent_analysis_stale_clearly_in_the_tree`,
`the_tree_says_which_file_the_project_is_in_and_whether_it_is_modified`,
`the_modified_marker_follows_the_engine_without_comparing_on_every_frame`). Snapshot: 24
(`tree-stale-and-modified`).

**UX ids.** UX-FR-07 (names from content; stale shown clearly), UX-PLOT-01 (a plot is an object in the tree).

**Port cost.** Small. It is a list; the work is a stable key per object and keyboard navigation. Delete and
rename are missing and would be new work (`analysis.remove` and `data.remove` exist but no screen calls them).

## 4. Project files, File menu, unsaved-changes dialog

**Files:** `projectfile.rs` 310, `projectmenu.rs` 113. Total 423.

**What it draws.** The File menu (New, Open, Save, Save as, Verify a project, quit where the program can
close) with the shortcuts Ctrl+N, Ctrl+O, Ctrl+S and Ctrl+Shift+S, and the modal question "save the
changes?" with three choices, shown before New, Open, a dropped project or closing the window would lose
work. The logic (`Guarded`: NewProject, OpenProject, OpenBytes with the program's token, Close) is in
`projectfile.rs`.

**Commands:** `project.save` (`projectfile.rs` 188), `project.new` (291), `project.load` through
`Engine::load_bytes` (251). **State:** `UiApp.file` (`FileState`: path, last saved revision, the pending
guarded action), not serialised; "modified" is `engine.revision()` against the saved one. The project file
itself is the engine's `Project` as bytes; the UI never reads or writes a file (the program does, section
19).

**Numbers:** none (`fmt::plain` for error sentences). **Tests:** `tests_project.rs`, 18 tests (6 harness calls): modified
marker, failed save, round trip, a file that is not a project, the three-choice question, drop, closing the
window, no quit in a browser, shortcuts, the token of a dropped file. Snapshots: 25, 26 (the question, light
and dark).

**UX ids.** None specific. It serves UX-REQ-03 (a refused file says why).

**Port cost.** Medium for what it looks like, large for what it implies: this is the contract with the host
(who picks files, who saves, when the window may close). The guard dialog and the dropped-file token need the
same behaviour in a webview and in a browser tab, where saving is a download and closing is not ours to do.

## 5. Data entry and paste

**Files:** `entry.rs` 1144, `entryflow.rs` 142, `entrygrid.rs` 121, `entrywide.rs` 484, `unitpick.rs` 136.
Total 2,027.

**What it draws.** "Type or paste data": a table with one row per sample (subject, time, concentration,
dose columns) or, for several subjects, a grid with the subjects in columns and a dose line on top; the
three unit cells with a small menu of the units the engine recognises (`unitpick.rs`); a "same dose for
every subject" field; the engine's checks of the units shown at once when a unit cell loses focus; the note
of what the last paste did; and "Create worksheet", which goes through the same checks as a file.

**Paste.** `egui::Event::Paste` is read in `entry.rs` 757 and lands in the cell that had focus at the last
frame. `split_paste`, `split_cells`, `quoted`, `is_table_paste`, `paste` and `fill_dose` (`entry.rs`) split
the text: tabs or other separators, quoted cells with a line break kept as one cell, a header line,
decimal commas. `entrywide.rs` (`read`, `WideGrid`, `long_rows`, `from_long`) recognises a wide table (times
down the first column, one column per subject, a dose line) and turns it into the long form. After that the
table is written as CSV text and goes through `data.preview` and `data.import`.

**Commands:** `units.check` (`entryflow.rs` 15, 42), `data.preview` (93), `data.import` (114).

**State.** `DataEntry` in `state.entry` (serialisable): name, subject toggle, the three units, rows,
common dose, layout, wide grid. Not serialised: `grid_focus`, `focus`, `review` (the `PendingImport` of the
last "Create worksheet"), `note`, `unit_checks`, `unit_focus`. The table is kept while the person looks
elsewhere and round-trips as data.

**Numbers:** none formatted for display; the typed text is kept exactly. **Tests:** `tests_entry.rs`, 17 tests
(5 harness calls): typed, pasted and imported data give the same worksheet and NCA; several subjects; a check waits
for a yes; a cell the engine cannot read; Ctrl+V spreads cells; wide table; quotes with a line break; unit
menu; subjects in columns. Snapshots: 43 to 47, 49 to 51, plus `docs/screens/type-or-paste-data.png`.

**UX ids.** UX-IMP-01 to UX-IMP-04 (the same checks as a file), UX-IMP-05 (units are data), UX-FR-05 (units
checked together), UX-FR-10, UX-FR-11 (duplicated times flagged before a run).

**Port cost.** High, and the first of the three costliest after the fit family. The text parsing is 1,600
lines of subtle rules with tests that only run through egui. The clipboard arrives as an egui event; in
React it is a `paste` DOM event with different line-ending and quoting behaviour, and in a Tauri webview the
spreadsheet's HTML flavour may come first. The sound move is to move the parsing into the engine (or a
shared crate behind the bridge) and keep only a grid in the front end; the alternative is to rewrite and
re-test about 1,600 lines in TypeScript.

## 6. Import

**Files:** `import.rs` 405. **What it draws.** The import preview, before anything is stored: how the file
was read (separator, decimal mark, header, the role and unit of each column, the first rows), the other
readings when the file is ambiguous (named, never hidden), the check a reading needs a "yes" for, and the
buttons Import and Cancel. Changing the separator reads the file again.

**Commands:** `data.preview`, `data.import` (called from `app.rs`, not from `import.rs`, which only draws and
returns `Action`s).

**State:** `UiApp.pending: Option<PendingImport>` (name, csv text, `ReadOptions`, the list of `Reading`s,
`needs_choice`, `error`, `chosen`, `acknowledged`). **Not serialised** (it is a field of `UiApp`, not of
`UiState`): closing the app in the middle of an import loses it, which is the intended behaviour (UX-IMP-02
says nothing is stored before a confirmation).

**Numbers:** the preview cells are the engine's strings. **Tests:** the first five tests of `tests.rs`:
`an_import_stores_nothing_until_it_is_confirmed`, `a_decimal_comma_file_is_read_with_the_right_marks_and_shown_before_import`,
`a_reading_with_a_check_is_not_imported_without_a_yes`, `changing_the_separator_reads_the_file_again`,
`a_file_that_is_not_text_is_refused_with_what_to_do`. Inline tests from line 343. Snapshots: 02, 03,
`docs/screens/import-french-csv.png`, `import-reading-with-a-check.png`.

**UX ids.** UX-FR-10 and UX-IMP-01 to UX-IMP-06 (detect, preview, never split a cell, refusal says what to
fix, units and display precision are data, required tests), UX-FR-11.

**Port cost.** Low to medium. The detection is in the engine (`data.preview`), so the screen is a table and a
few choices. The state machine (preview, choose a reading, acknowledge, import) is the thing to keep
intact.

## 7. Worksheet

**Files:** `sheet.rs` 419. **What it draws.** The page of the selected worksheet: its name, number of rows and subjects, the buttons
"New NCA analysis from this worksheet", "New model fit" and "New plot", the raw-plot card (section 8), a
"Columns" card (role and unit of each column, with the unit menu), and the editable table with a filter by
subject. A refused cell edit is kept in the cell and explained.

**Commands:** `data.describe`, `export.table` (read, in `refresh_sheet`), `data.set_cell`, `data.set_column`
(writes), `units.check` (unit cells, same menu as the entry table).

**State.** `UiApp.sheet: Option<SheetView>` (a cache of the last read) and `UiApp.rejected` are not
serialised. The cell-editing buffer and the subject filter live in **egui temporary memory**
(`sheet.rs` 63 to 103, 310 to 325): lost on a frame where the sheet is not drawn, and invisible to a test of
`UiState`.

**Numbers:** the cells are the engine's text (`fmt::exact` for edited values). **Tests:**
`a_refused_cell_entry_is_kept_and_explained`, `editing_the_data_marks_the_analysis_stale_and_run_again_refreshes_it`
in `tests.rs`; `the_worksheet_unit_cells_offer_the_same_list` in `tests_entry.rs`. Snapshot 04.

**UX ids.** UX-REQ-01 (the worksheet stays editable), UX-FR-07 (an edit marks dependants stale), UX-FR-11,
UX-IMP-05.

**Port cost.** Medium. A cell grid with edit buffers, error-in-cell, keyboard movement and a unit menu is
standard work in a DOM, easier than in egui, but a virtualised grid would be needed for long tables, and the
copy-out of the table has to keep columns and digits (one of the measures of T-130).

## 8. Raw plot

**Files:** `rawplot.rs` 779. **What it draws.** The data of a worksheet as a plot, in a card of the worksheet page (`rawplot::card`): one line
and marker shape per subject (five shapes: circle, square, diamond, up triangle, asterisk), linear or
semi-log, a clickable legend that hides or shows a subject, hover labels, and the note about points a
logarithmic axis cannot show.

**Commands:** none (it draws `RawData` built from the worksheet table of `export.table`).

**State:** the hidden subjects are in egui temporary memory (`rawplot.rs` 271 to 274), not serialised and
not in `UiState`. Linear or log is `state.log_axis`.

**Numbers:** axis ticks and hover values through `fmt::number` (2 uses). The hover value uses `powf` on a
log axis (`rawplot.rs` 205). **Tests:** inline tests from line 545; `every_screen_draws_without_a_panic_in_both_themes_and_both_axes`,
`the_semi_log_toggle_never_fails_on_zeros_and_says_what_is_hidden` in `tests.rs`;
`the_raw_data_card_and_the_palette_lead_to_a_plot` in `tests_plotpage.rs`. Snapshots 04a and 04b (six
subjects, light and dark).

**UX ids.** UX-FR-09 and UX-LOG-01 to UX-LOG-05 (log axis never fails, says what is hidden), UX-FR-13.

**Port cost.** Medium, and it duplicates plot logic: this is one of the **four** screen drawing paths
(section 18). The plot scene of T-133 should absorb it.

## 9. Plot panel (drawing shared by NCA, fit and simulation)

**Files:** `plot.rs` 496, `plotdata.rs` 294. Total 790. These are not screens; they are the plot code of the
right-hand panel, and they use `egui_plot`.

**What it draws.** `plot::profile` draws a concentration-time plot with `egui_plot`: a custom linear or
log axis (the log is done by hand in `plotdata.rs`, since `egui_plot` has no true log scale), points in
tones (`plot::Tone`: Observed, Other, Replaced, Fit, Selected, Start), the legend, and `plot::log_note`, the
collapsing note that lists the points a log axis cannot show with a "Switch to the linear axis" button
(UX-LOG-02). `plotdata.rs` is the display arithmetic: points, what the log axis can show, axis ranges
(`linear_ranges`, `decade_range`, `log_ticks`, `nice_ticks`, `log_label`), and which point a click selects.

**Commands:** none. **State:** none of its own (`state.log_axis`; the selected point is held by the NCA page).

**Numbers:** `fmt::number` (4) and `fmt::exact` (2) for tick labels and hover.

**Tests:** inline tests in `plotdata.rs` from line 155; `tests.rs` log-axis tests (UX-LOG-05). Snapshots
05 to 12 and 15 to 17 show the plot panel.

**UX ids.** UX-FR-03, UX-FR-09, UX-LOG-01 to UX-LOG-05, UX-LZ-01, UX-LZ-02.

**Port cost.** High if kept, and it should not be kept: the log axis and the tick logic are exactly what T-133
moves to a scene shared with the SVG export. See section 18.

## 10. Plot pages

**Files:** `plotpage.rs` 796. **What it draws.** A plot as an object (`plot.xy`), full width: source (a
worksheet, or a fit's observed against predicted, residuals, profile), which subjects, scale (linear, log or
both), layout (overlay or one panel per subject), title, then the panels, each with its own axes, an own
legend row, hover and click, and the "Export SVG" button. Panels are arranged by `columns(run, width,
min_width)`: two when linear and log are mixed, else up to three, limited by a 380 px token.

**Commands:** `plot.xy` (`plotpage.rs` 706), `export.plot` (775). Export asks the program to save the SVG as
a `Request::SaveFile`; `seal::svg_with_stamp` (788) adds the seal line.

**State.** `PlotPage` in `state.plot` (serialisable): `analysis`, `source`, `all_subjects`, `subjects`,
`scale`, `layout`, `title`. Not serialised: `drawn_title`, `label`, `status`, `result` (the `PlotRun`), `error`.
The page is a view of a stored `plot.xy` object, so most of what makes the plot reproducible is in the
engine's project.

**Numbers:** `fmt::number` (2). **Tests:** `tests_plotpage.rs`, 9 tests (3 harness calls): a plot makes an object,
options redraw it, it opens again from the tree and goes stale, a fit offers diagnostic plots as objects,
export asks the program to save an SVG, `the_export_colours_are_the_light_theme` (12 colour equalities with
`SvgColors`), `panel_axes_follow_the_guide_and_the_scale`, draws in both themes and narrow. Snapshots 52 to 58.

**UX ids.** UX-PLOT-01, UX-FR-13 (scale, title, series visibility), UX-FR-09, UX-LOG-02.

**Port cost.** High on its own and lower once T-133 lands: this is the only screen that already draws from a
plot answer, so it is the natural first user of the scene.

## 11. NCA

**Files:** `nca.rs` 1132. **What it draws.** The page of an analysis, read from the top: an editable "Worksheet data" section, the
subject, route and dose, units, options (AUC method, λz rules and the visible quality-flag thresholds, each
switchable off, plus "More options"), then the summary (the headline parameters with their flags), and the
details one click away ("All parameters", "Points of the profile", "Points left out"). The right-hand plot
panel (`nca::plot_panel`, line 860) holds the profile and the list of candidate λz fits, with "Back to
automatic selection" once a range is chosen by hand. The plot panel shows the profile with the λz line, the
replaced values and the used points; a click on a point includes or excludes it on a semi-log axis and
the engine is called again at once. A subject change forgets the hand-picked phase of the previous subject.

**Commands:** `nca.run` (`app.rs` 547), `analysis.get` (502), `analysis.run` (620). Every option change runs
`nca.run` again.

**State.** `NcaPage` in `state.nca` (serialisable): `analysis`, `worksheet`, `subject`, `route`,
`infusion_duration`, `dose_override`, `options` (as the engine echoed them after the last run). Not
serialised: `view` (the `NcaView` of the last result), `error`. The λz line is computed in the UI from the
result (`nca.rs` 957, standard `exp`).

**Numbers:** `fmt::number` (8), `words` (5), `exact` (3), `parameter_unit` (2), `fit_quality` (2),
`with_labels`, `parameter_label`. **Tests:** `tests.rs` NCA tests (a new analysis runs at once; an option
change asks the engine again and the numbers follow; clicking points chooses the terminal phase; a click the
engine refuses shows its message; editing the data marks the analysis stale; the subject change forgets the
hand-picked phase); inline tests from line 1070. Snapshots 05 to 12 and 33, and
`docs/screens/nca-linear.png`, `nca-semilog-hidden-zero.png`, `nca-manual-terminal-phase.png`,
`nca-two-subjects-flags.png`.

**UX ids.** UX-FR-02, UX-FR-03, UX-FR-04, UX-FR-05, UX-FR-12, UX-REQ-01, UX-REQ-03, UX-LZ-01, UX-LZ-02,
UX-LOG-01 to UX-LOG-05.

**Port cost.** High, and it is the screen of T-135 to T-137. The form is easy in any stack. The expensive
part is the click on a point on a semi-log axis with hidden points and a hand-built log scale: the mapping
from a pixel to a point must give the same point the engine accepts. A per-frame budget matters (the
measure is dragging the range over 50 subjects of 20 points). The λz line should come from the engine or the
scene, not be recomputed in each front end.

## 12. Model choice

**Files:** `modelpick.rs` 210, `modelinfo.rs` 774. Total 984. **What it draws.** Inside the fit and simulation
pages: input (IV bolus, IV infusion, oral first-order, oral zero-order, with an optional lag), one or two compartments, the
compartment diagram, the equation of the picked model, the parameter set for two compartments (clearances and volumes, micro-constants, or macro-constants), and one
editable row per parameter. `modelinfo.rs` holds `MODELS: [ModelInfo; 12]` (id, equation, meaning of each
parameter) and `diagram()`, painted with the egui painter (line 471).

**Commands:** none directly (the pages call `fit.initial_estimates`, `fit.evaluate`, `model.simulate`).
`describe()` is read for the parameter names (`modelinfo.rs` 676).

**State:** it is part of `FitPage` and `SimPage` (`compartments`, `input`, `lag`, `set`, `duration`); see
sections 13 and 14.

**Numbers:** `fmt::number` (1, in `modelpick.rs`). **Tests:** inline tests in `modelinfo.rs` from line 626;
`the_route_picks_the_model_and_the_starting_values_follow` in `tests.rs`; `tests_pk2.rs` (9 tests, 2 harness calls). Snapshots
13, 19, 34, 38.

**UX ids.** UX-FR-06 (a model is chosen by route and compartments; the id, the diagram and the equation are
shown), UX-FR-08 (later).

**Port cost.** Medium to high, mostly the table. The knowledge of 12 models with equations should come from
the engine (a `models` command or a field of `describe`) rather than be re-typed per front end. The diagram is
a small drawing: an SVG in the DOM is easier than the painter version.

## 13. Fit

**Files:** `fit.rs` 693, `fitform.rs` 466, `fitplots.rs` 184, `fitpreset.rs` 188, `fitresult.rs` 396,
`flow.rs` 306 (shared with the simulation page). Total 2,233.

**What it draws.** `fit.rs` is the page state and the fit setup; `fitform.rs` is the body read from top to
bottom (data, model, dose, starting values, weighting and options, the Fit button, then the results);
`fitplots.rs` is the plots beside the page (observed points with the live curve of the starting values and,
after a fit, the fitted curve, then the residual plots); `fitpreset.rs` reads the options presets from the engine ("Caladrius default",
"Reference conventions", and "Custom" once a field is edited); `fitresult.rs` is the results, summary first (convergence, estimates
with precision and both confidence intervals, goodness of fit, the flags in words), with the matrices,
eigenvalues, trace, partial derivatives and predicted values one click away; `flow.rs` is the conversation
with the engine.

**Commands:** `fit.initial_estimates` (`flow.rs` 127), `fit.evaluate` (165), `model.simulate` (182, 258,
290), `fit.run` (193; and `fitpreset.rs` 80), `analysis.run` and `analysis.get` for stored fits. **Each edit
of a starting value runs three commands** (generate or keep the values, redraw the curve, evaluate the
objective): the live preview of UX-REQ-02.

**State.** `FitPage` in `state.fit` (serialisable): `analysis`, `worksheet`, `subject`, `compartments`,
`input`, `lag`, `set`, `duration`, `dose_override`, `weighting`, `initial` (starting values by name),
`pending` (placeholders for two compartments, where the engine cannot generate values), `options`. Not
serialised: `view` (the `FitView`), `error`, `start_error`, `preview`. The preset label is derived by
comparing the options with the engine's presets.

**Numbers:** `fmt::number` (14 across `fitform.rs` and `fitresult.rs`), `fit_quality`, `words`, `exact`.
**Tests:** `tests.rs` fit tests (new fit prefills and draws the live curve; editing a starting value moves
the curve and the objective; fitting stores an analysis with precision and both intervals; a poor fit is
flagged with sentences; too few points; non-convergence says so; a singular fit names identifiability; a
negative starting value is refused; the engine's backticks do not reach the screen); `tests_fitpreset.rs`
(9 tests, 2 harness calls); `tests_pk2.rs`. Snapshots 13 to 20, 34 to 37, 40 to 42, and `docs/screens/fit-*.png`.

**UX ids.** UX-FR-02, UX-FR-03, UX-FR-04 (flags next to the number), UX-FR-06, UX-REQ-02, UX-REQ-03.

**Port cost.** The highest of the crate. About 3,700 lines with model choice and simulation. Why: the live
preview is a loop of three engine calls at each keystroke that must be cancelled or coalesced; the presets
come from a scratch engine; the two-compartment case has three parameter sets and placeholders; the results
have many tables; and the error text goes through `fmt::plain` to strip the engine's markup. In a web front
the engine runs in a worker, so every preview is asynchronous, which the egui code never has to think about.

## 14. Simulation

**Files:** `sim.rs` 506. **What it draws.** A model simulation: model choice, dose and parameter values, the
end time, the curve drawn by the engine, and "save as an analysis". Units come from the selected worksheet.

**Commands:** `model.simulate` (through `flow.rs`), `analysis.run` and `analysis.get` for a stored one.

**State.** `SimPage` in `state.sim` (serialisable): `analysis`, `worksheet`, `compartments`, `input`, `lag`,
`set`, `duration`, `dose`, `params`, `end`. Not serialised: `result`, `error`.

**Numbers:** `fmt::number` (1). **Tests:** `tests.rs` (`the_simulation_page_draws_the_engines_curve_and_saves_an_analysis`,
odd values, units from the worksheet, saving again updates instead of adding, the duration of an infusion);
`tests_pk2.rs`; inline tests from line 457. Snapshots 21 to 23, 38, 39, `docs/screens/simulation.png`.

**UX ids.** UX-FR-02, UX-FR-06, UX-REQ-03.

**Port cost.** Medium, mostly shared with the fit page. It is also the generator of the 50-subject dataset
of the T-130 measures, through the command line, not through this screen.

## 15. Settings

**Files:** `settings.rs` 634, `settingspage.rs` 225, `settingsflow.rs` 137. Total 996.

**What it draws.** One searchable page. Each row shows what the setting is, what it affects, its value as a
control, and its default with a way back to it. The groups: Interface (language, plot and interface theme), Numbers (decimal mark,
significant digits), New NCA (AUC method, fewest points in the terminal phase, terminal phase may include
Tmax), New fit (weighting, maximum iterations, convergence criterion) and System (the operating-system
locale, shown as a suggestion and never applied unseen). `settings.rs` describes the fields (`Field`, `Kind`)
so the page is drawn from data, and holds `Settings`, `NcaDefaults`, `FitDefaults`, `LocaleNote`.

**Commands:** none. A change is an `Action::SetSetting`, the same for the page, the palette and tests.
`settingsflow.rs` applies it (`fmt::set_display`, theme) and queues `Request::SaveSettings`.

**State.** `Settings` is serialisable and is **the only state the apps persist**. `state.settings_query`
(the search box) is in `UiState`.

**Numbers:** `fmt::exact`, `fmt::exact_point`, `fmt::parse_number`, `fmt::set_display`. **Tests:**
`tests_settings.rs`, 9 tests (5 harness calls): the page lists every setting with its default, search, number display
follows without touching data, refused values, round trip, locale suggestion, new analyses use the changed
defaults. Inline tests from line 472 and 201. Snapshots 30 to 32.

**UX ids.** UX-FR-14, UX-SET-01, UX-FR-10 (import does not follow the OS locale).

**Port cost.** Low. It is a form drawn from a field list. The field list should be exported as data
(it already is a Rust structure) so each front end draws the same page. The one real cost: number display
changes must reach every formatted number on screen at once.

## 16. Verify a project

**Files:** `verify.rs` 408. **What it draws.** The report of replaying a project file: the verdict on top,
each step with the start of its seal and its status, the first difference with both seals, the project line
when a stored result was edited though every step matches, and a message when the file was saved before seals
existed. Buttons: verify another file, close. Nothing in the open project changes.

**Commands:** `project.replay`, but through `replay_bytes` on a scratch `Engine::new()` (`verify.rs` 42 to 43),
returning the typed `replay::Report`. Not an `execute` call: a bridge has to expose it.

**State.** `UiApp.verification: Option<Verification>`, not serialised; the page without a report is the
welcome page.

**Numbers:** text only (`fmt::plain` 4). **Tests:** `tests_verify.rs`, 11 tests (8 harness calls), one per verdict:
verified, first difference, edited result, saved before seals, not a project. Snapshots 59 to 63,
`docs/screens/verify-project*.png`.

**UX ids.** None in `specs/ux.md`: the spec has no requirement for verifying a project file. The screen
shows the replayable history and the seals, which is a claim of the project, not a UX requirement.

**Port cost.** Low for the screen, medium for the data: the report structure has typed enums, so the schema
should come from `describe()`. This is the screen that best shows what Caladrius is, so it is worth
getting right in every stack.

## 17. Command palette

**Files:** `palette.rs` 552. **What it draws.** Ctrl+K opens an overlay with a search box and a ranked list: the
actions of the application (new analysis, open, save, verify, switch theme, settings), the registry (all 25
commands from `describe()`, with id, title and one line), and the worksheets and analyses by name. Arrow
keys move, Enter runs, Escape closes. Typing a verb puts the action before the command of the same name.

**Commands:** `project.describe` (388). `command_action` maps registry ids to pages: `data.import` and
`data.preview` to Open CSV; `nca.run` to New analysis; `fit.run`, `fit.initial_estimates`, `fit.evaluate` to
New fit; `model.simulate` to New simulation; `plot.xy` to New plot; `export.plot` to the SVG export;
`project.save`, `project.load`, `project.new`, `project.replay`, `project.describe` to the file actions;
`analysis.run` runs the analysis in view again; `analysis.get` selects it; `data.describe`, `data.set_column`,
`data.set_cell` open the selected worksheet. The remaining ids give the "no page yet" notice.

**State.** `PaletteState` in `state.palette` (serialisable): `open`, `query`, `selected`. The entries are
rebuilt each time from the registry (read once into `UiApp.commands`).

**Numbers:** none (`fmt::plain`). **Tests:** `tests_palette.rs`, 10 tests (3 harness calls): every registered command
is listed, an NCA from a worksheet in two actions, keyboard only, escape and button, the selection is the
context, arrow keys, verb ranking. Inline tests from line 490. Snapshots 27 to 29,
`docs/screens/command-palette.png`.

**UX ids.** UX-FR-01.

**Port cost.** Low to medium. The ranking and the mapping are small; the cost is that the palette is how
four commands are reachable at all and how every other command is documented, so the port must keep the
registry-driven list rather than a hand-written menu.

## 18. Plots

Input of T-133. The same plot is drawn by **four screen paths** and **one SVG path**, and they differ.

**The four screen paths:**
1. `rawplot::raw_data`, from the worksheet table (section 8).
2. `nca::profile_points`, from `NcaOk` (section 11), drawn by `plot::profile`.
3. The fit and simulation curves, drawn by `plot::profile` and `fitplots.rs`.
4. `plotpage::panel_axes` and `draw_series`, from `PlotRun` (section 10).

**The SVG path:** `export.plot` in `crates/caladrius-engine/src/commands/plot.rs` (233), drawn by
`crates/caladrius-engine/src/svg.rs` (797 lines). **Only the fourth screen path, the plot-page object
(`plot.xy`), can be exported.** The NCA, fit and simulation profile plots cannot be exported as SVG.

| Aspect | Screen | `export.plot` SVG (`svg.rs`) |
|---|---|---|
| Linear range | `plotpage::linear_range` always includes 0 (`lo.min(0.0)`), from `lo - 3 %` span to `hi + 5 %`; residuals symmetric at 1.15 times the reach. The raw plot and the profile use `plotdata::linear_ranges` (from `min(0, ...)`, `+ 5 %`) plus margins (`raw_axes`: x `-2 %`, y `-5 %`). | `linear()` pads 5 % both sides and includes 0 only for the Zero guide or when a BLQ point exists, and never on x. |
| Tick step | `plotdata::nice_ticks`, target 6 (step at least span/6). | span/5. |
| Tick labels | `fmt::number`: 4 significant digits, decimal mark and digits from settings, scientific outside 1e-3 to 1e5. | `tick_label`: decimals from the step, `{:e}` for 1e6 and above or below 1e-4, always a decimal point. |
| Log range | `decade_range` floor and ceil, a single decade gives `[lo, lo + 1]`; plot pages and the raw plot add `-0.12` and `+0.04` decades, with the BLQ marker at `d0 - 0.06`. `plot::profile` uses the decades with no BLQ margin. | `logarithmic`: a single decade gives `+-0.5` around it, then `lo - 0.12`, `hi + 0.04`; BLQ at `d_lo - 0.06`. |
| Log ticks | `log_ticks`: major decades plus minor lines at 2 to 9 (minor at half alpha); labels `log_label` (-3 to 4 as plain powers, else `1e{n}`). | Decade lines only (at most 40); `decade_label` (-4 to -1 fixed decimals, 0 to 5 plain, else `1e{k}`). So 1e-4 reads "1e-4" on screen and "0.0001" in the SVG; 1e5 reads "1e5" and "100000". |
| Hidden points | A collapsing note with the count and a list, a "Switch to the linear axis" button (profile, fit, simulation, raw), an empty state. Plot pages show `run.notes` sentences and "No value above zero: nothing to show on a logarithmic axis." | `series()` drops the points silently (`Scale::value` returns `None`); `PlotRun.notes` are not written; the same "No value above zero..." sentence is printed only when nothing remains. |
| Colours | Theme tokens, light and dark (`theme.json`). | Fixed light constants (`SvgColors`, `COLORS`). Only `the_export_colours_are_the_light_theme` (`tests_plotpage.rs`, 12 equalities) ties them. The dark theme is never exported. |
| Roles | Tones: Observed, Other, Replaced, Fit, Selected, Start. | `SeriesRole::{Subject{slot}, Observed, Predicted}` only. The NCA "Replaced value", "Used for λz" and "Start" curve have no SVG role. |
| Markers | Five shapes per subject (circle, square, diamond, up triangle, asterisk), sizes from tokens 4, 5, 6.5 px (plot pages use Medium, 5). | Circle, rect, diamond, triangle, cross (`slot % 5`), fixed radius 4 (BLQ 5.2). |
| Line widths | Tokens 1.0, 1.5, 2.0. | Literals 2 and 1.5 (same values, separate sources). |
| BLQ marker | Hollow down triangle, at 0 on linear and in the log margin. | The same shape and rule. |
| Legend | `egui_plot::Legend` for profiles; a custom row on plot pages (shown for two or more names or a BLQ point); on the raw plot a clickable legend that hides subjects (temporary state). | One legend below the panels, wrapped at the figure width (7 px per character estimate), same show rule. |
| Layout | `columns(run, width, min_width)`, responsive: two when mixed linear and log, else up to three, limited by 380 px. | Panel 420 by 300 by default, `width` and `height` parameters (the UI passes only `{analysis}`), same two or three rule. |
| Interaction | Hover labels and click to select. | None. |
| Zero guide | Drawn regardless of the axis. | Skipped on a log axis. |
| Identity diagonal | From `axes.x.0` to `axes.x.1`. | From `sx.lo` to `sx.hi`, dashed "6 4". |
| Fonts | egui default fonts. | `font-family="sans-serif"`. |
| Titles | A heading plus small panel titles above. | The title plus in-panel titles. |
| Computed in the UI | The λz line (`nca.rs` 957): intercept, slope, 60 steps, standard `exp`. | Not drawn at all. |

**Where this leads for T-133.** The differences that matter for a shared scene, in order: (1) the log
axis range and ticks (same data, different decade rules and labels); (2) hidden points (the screen explains
them, the SVG hides them); (3) the roles that exist only on screen; (4) tick step and labels; (5) colours
from tokens versus constants. Anything the scene decides (ticks, decades, hidden-point sentences) will change
either the screen or the SVG; the card says the SVG stays byte-identical, so the screen moves toward the SVG
in the cases where they differ, or the scene takes a "style" input.

## 19. Platform code

Input of T-134. What `apps/caladrius` (desktop) and `apps/caladrius-web` (browser) do that is not interface.

### Desktop: `apps/caladrius/src` (main.rs 668, config.rs 237, projectio.rs 116)

- **`main.rs`:** `Desktop` wraps `UiApp`. `serve` answers every `Request`: `PickCsv`, `OpenProject`,
  `VerifyProject`, `SaveProject {suggested, bytes, ask}`, `Close`, `SaveSettings`, `SaveFile`, using `rfd`
  native dialogs and `std::fs`. It reads the system clock and gives it to the UI with `UiApp::with_clock` (the
  UI has no clock access, which a test enforces). It handles dropped files (a project or a CSV, with a token
  queue for the unsaved-changes question), keeps the window title in step with the project, and answers
  `CancelClose`. It decodes `logo::ICON_256` into the window icon. The window is 1400 by 900 points,
  minimum 900 by 600, on eframe with wgpu (dx12, metal, vulkan). The command line takes `--help`, `--version`
  and a file argument.
- **`config.rs`:** the configuration directory per operating system, `settings.json`, and the system locale
  read from the environment or, on Windows, from the registry (by running the `reg` command).
- **`projectio.rs`:** project file read and write, and the default extension.
- **Clipboard:** no code. The paste arrives as `egui::Event::Paste` inside the UI (section 5).
- **Fonts:** none loaded. eframe's `default_fonts` (egui's built-in fonts) only. The design direction of
  T-132 (Amiri, Alegreya Sans, IBM Plex Mono) would need font files in every stack.

### Browser: `apps/caladrius-web` (host.rs 473, browser.rs 250, main.rs 31, index.html 44)

- **`host.rs`** (platform-neutral, testable): a `Browser` trait (`pick`, `download`, `store_settings`,
  `take_incoming`, `set_title`, `set_unsaved`) and `Web<B>`, which does for the browser what `Desktop::serve`
  does for the desktop.
- **`browser.rs`** (wasm32 only; the `web-sys` glue): a hidden file input for picking, a Blob and anchor for
  downloading, local storage key `caladrius.settings`, `navigator.language` for the locale, `js_sys::Date::now`
  for the clock, `onbeforeunload` when there are unsaved changes, `document.title`, dropped files, and
  `eframe::WebRunner` on the canvas `#caladrius` (glow, WebGL).
- **`index.html`:** loading text ("about 2 MB"), favicons, a CSS dark mode, and a module script importing
  `./caladrius.js`. `xtask web` runs the cargo build with the `web` profile, `wasm-bindgen --target web`, an
  optional `wasm-opt -Os`, and copies the icons.
- Every save in the browser is a download; `Request::Close` does nothing; `can_quit` is false.

### What a new front end must provide

| Capability | Desktop today | Browser today | Bridge consequence |
|---|---|---|---|
| Pick and read a file | `rfd` + `std::fs` | file input | the front end reads bytes and sends them as a command argument |
| Save a file | `rfd` + `std::fs` | download | the bridge returns bytes; the front end saves |
| Settings storage | `settings.json` | local storage | the front end stores `Settings` as JSON |
| System locale and clock | env or registry; system clock | `navigator.language`; `Date.now` | passed in, never read by the engine or the UI |
| Dropped files | window events | DOM events | the same project-or-CSV decision |
| Close guard | `CancelClose` | `onbeforeunload` | a "modified" flag from the engine revision |
| Clipboard | egui event | egui event | a DOM paste event in a webview |
| Fonts and logo | egui defaults; PNG in the binary | same, in wasm | tokens and font files in the app |
| Engine | in process | in wasm, in the page | a Web Worker in a browser; Tauri command on the desktop |

## 20. Costliest to port, and a bolder next step

1. **Fit family** (sections 12, 13, 14): about 3,700 lines. A hard-coded table of 12 models, a painted
   diagram, a three-command live preview at each edit, presets read from a scratch engine, the
   two-compartment parameter sets, and the results tables.
2. **Data entry and paste** (section 5): about 2,000 lines of subtle text parsing plus the clipboard event.
3. **NCA page and its plot** (sections 11 and 9): 1,132 lines plus the plot stack. Click-to-select on a
   semi-log axis with hidden points and a hand-built log scale.

**A bolder next step** (proposed, not started): make the plot scene an engine output, including the λz line and
the replaced, selected and start roles, so the screen and the SVG are one drawing; and generate the
front-end types from the `result_schema` of `describe()`. Together they remove the largest duplicated logic
(four screen plot paths) and the largest hand-written model of the engine's answers.
