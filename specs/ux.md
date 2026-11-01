# User experience: workflow and requirements

Card T-017, revised by T-129. The interface requirements of Caladrius, in its own words. Sources: `AGENTS.md` section 7 and the rules of `specs/nca.md`, `specs/models.md` and `specs/fit.md`. The interface agent works from this file and the theme tokens.

## 0. How to read this file

**Status tags** (`specs/README.md`):
- `documented, untested`: stated here and not yet covered by a test of the interface;
- `assumed`: a design decision for Caladrius, to be tested by the interface agent;
- `confirmed by oracle` is not used here: the oracle covers numbers, not screens. A rule that has a numeric core cites the rule of `specs/nca.md`, `specs/models.md` or `specs/fit.md` that carries it.

**Rule ids.** `UX-<AREA>-<nn>`. Areas: FR (interface requirements), REQ (required improvements of `AGENTS.md` section 7), IMP (import), LOG (logarithmic axes), LZ (λz selection), ERR (errors and messages), SET (settings).

**Terms.** "Worksheet" is a table of typed columns. "Analysis" is an NCA or a fit with its inputs and results.

## 2. Interface requirements

Each item states what Caladrius does and how an interface test would check it.

### UX-FR-01 New analysis is one button and the command palette
- Caladrius: one "New analysis" button that is always visible, and a command palette (every command has a stable id, golden rule 4: `nca.run`, `fit.run`, `plot.xy`, `data.import`); the palette also offers "new analysis from this worksheet".
- Test: from an open worksheet, an NCA is created in at most two actions (a click and a confirmation, or a palette entry), and the same command runs from the CLI.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-02 One page for an analysis
- Caladrius: one page read from top to bottom: data, model or route and dose, units, options (AUC method, weighting, λz rules), initial estimates for a fit, with the plot always visible. Advanced options are in a collapsible section on the same page, not a separate tab. The results appear on the same screen (UX-REQ-01).
- Test: every option of the command's parameter structure is reachable on that page without leaving it; the page is serialisable as the command's parameters.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-03 A summary first, details on demand
- Caladrius: a summary first (for NCA: the headline parameters with their flags, the profile plot with the λz line and the chosen points; for a fit: parameters with precision, the fit plot, residuals and goodness of fit), details on demand (every table of the engine's output, `specs/fit.md` FIT-OUT-01 to 11).
- Test: the summary is visible without a click after a run; each detail table is one click away and can be exported.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-04 A poor fit is not flagged
- Caladrius: the flags of `specs/fit.md` FIT-FLG-01 and `specs/nca.md` NCA-LZ-12b/12c are shown next to the number they concern, with the text that says what to check; thresholds are options. A flag never changes a number.
- Test: a fit with a parameter CV% above the threshold shows the flag in the summary; the replaced-value flag of `specs/nca.md` appears on the profile of case `edge_blq_set`.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-05 Dose, concentration and time units are checked together
- Caladrius: dose, concentration and time units are checked together before a run (`specs/nca.md` NCA-UNIT-02): a mass dose with a molar concentration asks for a molecular weight; a time unit different between data and infusion duration is an error; derived units are shown with every result. Units are data attached to the columns, never read from the locale.
- Test: dose in mg with concentration in nmol/L shows the warning and the offer before the run; results carry units.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-06 A model is chosen by route and compartments
- Caladrius: choose by route and number of compartments; the model id of `specs/models.md` MOD-GEN-05 (`pk1.oral_1_lag`, …) appears with its diagram and equation, and the parameters it needs.
- Test: choosing "oral, first order, with lag" selects `pk1.oral_1_lag` and shows the equation of MOD-AB1-05.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-07 Objects are named from their content
- Caladrius: objects are named from their content (for example "NCA, oral 100 mg, subjects 1 to 12"), and an analysis whose input changed after its last run shows its results as stale, clearly.
- Test: editing a worksheet cell marks every dependent analysis stale; a new analysis gets a content-based name.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-08 User models need a fixed-format block language (later step)
- Caladrius: readable equations with named parameters and immediate error messages, in the step after one-compartment models.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-FR-09 A logarithmic axis accepts a zero or a negative value
- Caladrius: UX-LOG-01 to UX-LOG-04.
- Status: `documented, untested`
- Sources: design of the logarithmic axis.

### UX-FR-10 Import does not follow the operating-system locale
- Caladrius: UX-IMP-01 to UX-IMP-06.
- Status: `documented, untested`
- Sources: UX-IMP-01.

### UX-FR-11 Duplicated times are flagged before a run
- Caladrius: the import preview and the data page flag rows that share a time within a profile, name them, and offer the explicit actions (keep the first, average, treat as separate profiles); nothing is merged without the user's choice.
- Test: a profile with two rows at time 0 shows the flag and the choices before any run.
- Status: `assumed`
- Sources: `specs/nca.md` NCA-DAT-02.

### UX-FR-12 Quality-flag thresholds are visible
- Caladrius: the quality flags of `specs/nca.md` NCA-LZ-12b have visible defaults (adjusted R² 0.9, span ratio 2, extrapolated AUC 20 %, 3 points) on the analysis page, switchable off, and the result says which were applied.
- Test: the flag thresholds are visible on the page; turning one off removes the flag and nothing else.
- Status: `assumed`
- Sources: `specs/nca.md` NCA-LZ-12, NCA-LZ-12b.

### UX-FR-13 A plot is styled from the theme
- Caladrius: a plot is styled by a theme token file (light and dark, `AGENTS.md` golden rule 7) with a few visible controls (scale, title, series visibility); advanced styling is data in the plot's serialisable state, not a deep tree.
- Test: the plot's state round-trips as data; switching theme changes no hard-coded colour.
- Status: `assumed`
- Sources: `AGENTS.md` golden rule 7.

### UX-FR-14 Settings are one searchable page
- Caladrius: a settings page with only what the application uses (language, number and date display, default options of commands, plot theme), searchable, each default showing its value and what it affects.
- Test: every setting is a field of a serialisable settings structure; no setting is read from the operating-system locale without being shown.
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-PLOT-01 A plot is an object of the workflow (T-110)
- Caladrius: an XY plot is an analysis object like an NCA or a fit (`plot.xy`): in the project tree, named from its content, saved with the project, marked out of date when its worksheet changes or when the fit it shows is run again or goes stale, redrawn by "Run again". Its page holds the few decisions that matter, top to bottom: the data (a worksheet, or a fit already run with one of its four views: observed and fitted curve, observed vs predicted, residuals vs time, weighted residuals vs time), the subjects (all, or ticked), the layout (overlaid, or one panel per subject), the scale (linear, semi-log, or both side by side; residuals are linear only; observed vs predicted is log-log), a title; the drawing follows at once. Export: SVG of the drawing in the light colours (`export.plot`), and the points as a table (`export.table plot.points`). The raw-data card of the worksheet stays the quick look, with a button that keeps it as a plot.
- Test: `caladrius-engine` `tests_plot`, `caladrius-ui` `tests_plotpage`.
- Status: confirmed by tests. Later cards: plots of a parameter across subjects (box plot, bars), histogram, QQ plot, scatter matrix, categorical XY, PNG export.

## 3. Required improvements (`AGENTS.md` section 7)

### UX-REQ-01 One screen from data to results for NCA
Data (table and plot), route and dose, options, and the NCA result with its profile plot are on one screen; changing an option re-runs the engine immediately (the NCA is fast) and updates the result and the flags. The worksheet of the source data stays editable on the same screen.
- Status: `assumed`
- Sources: `AGENTS.md` section 7; UX-FR-02, UX-FR-03.

### UX-REQ-02 Live curve while the initial estimates change
For a fit, the predicted curve of `specs/models.md` is drawn over the data and updates as each initial estimate is edited (spinner, slider or typed value); the generated initial estimates of `specs/fit.md` FIT-INI-01 to 04 are the starting values and can be edited; the objective (WRSS) is shown.
- Status: `assumed`
- Sources: `AGENTS.md` section 7; `specs/fit.md`.

### UX-REQ-03 Every error says what to fix
An error message names the field, the value found and the action that fixes it (`AGENTS.md` golden rule 6). The engine's errors already carry the parameter or time; the interface adds where to click.
- Status: `assumed`
- Sources: `AGENTS.md` sections 3 and 7.

## 4. λz selection

### UX-LZ-01 Click points on the curve to choose the λz range
The profile plot shows, on a semi-log view, every eligible point (`specs/nca.md` NCA-LZ-02, NCA-LZ-02c) and the points used by the current fit; the user clicks a point to include or exclude it, or drags to select a time range; each change calls the engine with a manual selection (NCA-LZ-08) and updates λz, half-life, R², adjusted R², span ratio and the flags at once. The table of candidate fits (NCA-OUT-01) is shown beside the plot and a click on a row selects that candidate. A "back to automatic" action restores the automatic choice.
- Status: `assumed`
- Sources: `AGENTS.md` section 7; `specs/nca.md` NCA-OUT-01, NCA-LZ-08, NCA-LZ-12b.

### UX-LZ-02 Replaced values are shown as such
Points whose value was replaced by the BLQ or missing-value policy (`specs/nca.md` NCA-DAT-07b) are drawn with a distinct marker and a tooltip "replaced value"; if the fit uses some, the flag `replaced_points_in_lambda_z` is shown; the option to exclude them (NCA-LZ-02d) is one click away.
- Status: `assumed`
- Sources: `specs/nca.md` NCA-DAT-07b, NCA-LZ-02b, NCA-LZ-02d, NCA-LZ-12c.

## 5. Logarithmic axes

### UX-LOG-01 A logarithmic axis never fails
Switching an axis to a logarithmic scale never raises an error or leaves the axis without a range. Points that cannot be drawn on a log axis (a concentration or time equal to 0, or negative) are left out of the logarithmic view only; no value of the data is changed.
- Status: `documented, untested`
- Sources: design of the logarithmic axis.

### UX-LOG-02 The plot says what is hidden and why
A visible, non-modal note on the plot gives the number of hidden points and the reason, for example "2 of 9 points not shown on a log axis (zero or negative values)"; a click on the note lists the hidden points (time and value) and offers to switch back.
- Status: `documented, untested`
- Sources: design of the logarithmic axis. Worked example below.

### UX-LOG-03 The range comes from the remaining points
The axis limits are computed from the points that remain, rounded outward to whole decades for the tick labels (lower limit the decade at or below the smallest remaining value, upper limit the decade at or above the largest); with a single remaining point the range is one decade around it. If no point remains (all zero or negative), the plot shows an empty state with the message of UX-LOG-02 instead of an axis.
- Status: `assumed`
- Sources: design of the logarithmic axis; design choice for the rounding.

### UX-LOG-04 Switching back restores everything
Returning to the linear scale shows every point again, with the previous range; the toggle can be repeated any number of times without changing the data or the analysis results. A logarithmic time axis follows the same rules for a time of 0.
- Status: `documented, untested`
- Sources: design of the logarithmic axis.

### UX-LOG-05 Required interface test
For every oracle dataset whose profiles start with a zero concentration, the interface test toggles the concentration axis to logarithmic and back and checks: no error, the count of hidden points equals the number of non-positive values, the range comes from the positive ones, and the restored linear view equals the original. The datasets in `oracle/data/` with a first concentration of 0 are: `theoph` (subjects 2 to 6, 8, 9, 11 and 12), `synthetic_lz` (all four), `edge_blq` (all six), `edge_missing` (all five, with empty values also left out), `edge_negative` (both, with negative values), `edge_oral` (subjects 1, 2, 3, 6), `edge_iv` (subjects 2 and 3), `edge_infusion` (all three). `indometh` has no zero and checks the case where nothing is hidden. The fit datasets are the same profiles.
- Status: `assumed`
- Sources: design of the logarithmic axis; `oracle/data/*.csv`.

**Worked example (UX-LOG-02, UX-LOG-03).** Profile 1 of `edge_blq` (time 0, 0.5, 1, 2, 4, 6, 8, 12, 24 h; concentrations 0, 2.1, 6.3, 0, 4.4, 3.1, 2.2, 1.1, 0.52): two points are zero (times 0 and 2), so seven of nine are shown and the note reads "2 of 9 points not shown on a log axis (zero or negative values)". The remaining values run from 0.52 to 6.3, so the axis is drawn from 0.1 to 10 (decades below 0.52 and above 6.3). Switching back shows nine points on the linear axis. A profile in which every concentration is zero shows the empty state and the note "9 of 9 points not shown", with no axis.

## 6. Import of tabular data

### UX-IMP-01 Detect the separator and the decimal convention
When a file is opened, Caladrius reads a sample (the first rows) and determines the field separator (comma, semicolon, tab, vertical bar or runs of spaces) and the decimal mark (point or comma) by consistency, not by locale: a candidate pair is admissible when every row has the same number of fields as the header and every cell of a column that looks numeric parses as a number under it. If a separator that is not the comma gives consistent rows, and commas occur only inside fields that parse as decimals with one comma, the comma is the decimal mark (a semicolon file with "0,25" is a decimal comma). If exactly one admissible pair exists it is proposed; if several do, the preview shows the alternatives and the user chooses; if none, the file is refused with the reasons (UX-IMP-04).
- Status: `assumed`
- Sources: design of the import.

### UX-IMP-02 Preview before anything runs
The parsed table is shown with its row and column counts, the detected separator and decimal mark (editable), the detected units if the header carries them, and the first and last rows, before the data enters the project. Nothing is imported and no analysis is run until the user confirms. The preview is the same screen for a pasted table.
- Status: `assumed`
- Sources: design of the import.

### UX-IMP-03 Never split or lose a numeric cell
An import is refused, not repaired, when a numeric cell would be split into two cells, merged with another, truncated, or lost: a row with more fields than the header, a numeric column with a non-numeric cell (other than an explicit missing-value marker), a decimal mark that differs between cells of the same column, a thousands separator mixed with a decimal comma, or an unterminated quoted field.
- Status: `assumed`
- Sources: design of the import.

### UX-IMP-04 The refusal says what to fix
The message names the first offending row and column, shows the cell as read, and offers the way out: change the separator, change the decimal mark, or fix the file (for example "row 4, column 2: '25,5' contains a comma but the decimal mark is a point; choose decimal comma or edit the file").
- Status: `assumed`
- Sources: `AGENTS.md` section 7.

### UX-IMP-05 Units and display precision are data
The unit of a column is a field of the data (read from the header if present, or chosen by the user), never inferred from the locale; the number of decimals shown is a display setting of the column and never rounds the stored value. Typing a number into a cell accepts both marks according to the shown convention and says which it understood; an entry that cannot be read is kept in the cell and marked, not discarded.
- Status: `assumed`
- Sources: design of the import.

### UX-IMP-06 Required tests
(a) A semicolon file with decimal commas and a comma file with decimal points, holding the same profile, give identical tables. (b) A file in which the same text "0,25" could be read two ways is shown with both readings and not imported until the user picks one. (c) A row with an extra field is refused with the row number. (d) The oracle datasets, exported by the test in three conventions (comma and point, semicolon and comma, tab and point) and re-imported, return the oracle values exactly (to the 15 digits written in `oracle/data/*.csv`).
- Status: `assumed`
- Sources: design of the import.

**Worked examples (UX-IMP-01).** (1) Text `time;conc` then `0,25;3,4`: the semicolon gives two fields in both rows; commas occur inside fields that parse as decimals with one comma; the pair (semicolon, decimal comma) is the only admissible one and yields time 0.25, concentration 3.4. (2) Text `time,conc` then rows such as `0,25`, `0,5`, `1,2`: with the comma as separator every row has two fields like the header, so by consistency alone that reading is admissible, and a quarter can be read as time 0 and concentration 25 when the author meant a decimal comma, or a semicolon was lost on the way. The reading is therefore subjected to plausibility checks on the result: within a profile the time column must be non-decreasing and must have as many distinct values as rows (here it would be 0, 0, 1 and repeats zeros), and no column may consist only of the digits that follow a mark. A reading that fails a check is never imported silently: the preview shows it next to the alternative reading and asks for a choice. The checks are a heuristic (UX-O-02).

## 7. Settings

### UX-SET-01 Locale is displayed, not assumed
The interface language and the number display follow the user's choice, with the operating-system locale only as the initial suggestion, shown in the settings. Changing the locale never changes stored data or what a file import understands (UX-IMP-01).
- Status: `assumed`
- Sources: UX-IMP-01.

## 8. Open items

| id | item | rule | who and how |
|---|---|---|---|
| UX-O-02 | The ambiguity heuristic of UX-IMP-01 (a file whose only admissible reading is the wrong one) is specified; a later public example can extend the checks | UX-IMP-01 | a public sample, if one is added |
| UX-O-03 | UX-FR-11 to UX-FR-14 are design requirements of this file | UX-FR-11 to 14 | interface tests |
