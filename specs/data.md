# The population dataset (event table): behaviour specification

Card T-147 (brick 2 of iteration 3, T-139), written by the `reader` agent on 2026-11-07, in its own words, before any oracle or code. It specifies the table that holds several subjects, each with its doses, observations, BLQ flags and covariates, and how today's single-subject worksheet maps into it. The maximum-likelihood fit that reads one profile of such a table is in `specs/fit.md` section 13; the NCA is in `specs/nca.md`. Sources are cited by the keys of `specs/sources.md` (section 13 holds the keys S-39 to S-47 added for this file); `D` is a design argument shown here. **Update of 2026-11-07 (card T-151).** The dataset oracle (`oracle/expected/data/`, card T-148; the tests are in `crates/caladrius-project/tests/oracle_data.rs`, 122 tests) and the dataset layer (`crates/caladrius-project/src/dataset.rs`, card T-150) now exist. Where the code departs from a rule below, or settles a point the rule left open, the rule says so and cites the card thread. The statuses of the rules that card did not touch are not regraded.

## 0. How to read this file

**Status tags** (see `specs/README.md`): `documented, untested` (found in an allowed source, not exercised), `assumed` (a design choice of Caladrius, to become a test before it is relied on), `later` (described by a source but outside the current scope: listed, not specified, no test). Rules are regraded `confirmed by oracle` only when a versioned test reproduces them, clause by clause (DAT-ERR-07 is the first; DAT-ERR-08 beside it is `assumed`). The oracle files name the rules they exercise in their `rules` field, and the open items of section 12 carry the state of the oracle work.

**Rule ids.** `DAT-<AREA>-<nn>`. Areas: LAY (layouts), COL (columns), REC (recognition of names), TYP (record types), CMT (compartments), UNI (units), ORD (order), ERR (errors), COV (covariates), CEN (missing values, BLQ, censoring), LEG (the legacy single-subject file), USE (what the analyses do with the table), VOC (vocabulary), LAT (later).

**Clean room.** The conventions are those of the open tools: `rxode2` and `nlmixr2` event tables, `saemix` data objects, `nlme` grouped data. Their documentation was read through fetched pages and search summaries except where `specs/sources.md` says "read in full" (the `nlme` help pages and the `Theoph` help page were read in full on this PC). The documentation of commercial programs was not used. Where a convention is known only through a summary the rule says so in its `Sources:` line.

**What exists today** (read from `crates/caladrius-project`, no rule). A table is imported from CSV (delimiter comma, semicolon or tab, decimal comma allowed with the last two); a header can carry a unit, `Conc (ng/mL)` or `Time [h]`. The role of a column is guessed from its name: time (`time…`, `t`, `hours`, `hour`, `hr`, `h`, `temps`, `heures`, `heure`), concentration (`conc…`, `c`, `cp`, `dv`, `cobs`), subject (`subject`, `subj`, `id`, `subjectid`, `patient`, `animal`, `individual`, `sujet`, `individu`, `volontaire`), dose (`dose…`, `amt`, `amount`), route (`route`, `admin`, `administration`), otherwise `other`; the match is on the lower-case letters and digits of the name; a column whose content does not fit its guessed role is left `other` with a note; at most one column has each role. Cells written as below the limit of quantification are stored as 0 with a BLQ flag (`specs/nca.md` NCA-DAT-05c). A reading check lists `time_not_increasing` and `duplicate_times` per subject. A worksheet gives `subjects()` and `profile(subject)`; with several subjects and none named the profile is refused (`subject_required`); a subject with two different values in the dose column is refused ("one dose per profile"). NCA runs one analysis per subject when no subject is named; a fit needs one subject. Results are per subject and an error is a result, not an abort.

## 1. Conventions of the open tools, side by side

(No rule.) What each tool calls the pieces of a dosing-and-observation table, as far as the sources read show.

| piece | `rxode2` / `nlmixr2` event table | `saemix` data object | `nlme` grouped data | Caladrius today |
|---|---|---|---|---|
| subject | `ID` | `name.group` (a column name) | the grouping factor after `\|` in `resp ~ time \| Subject` | `subject`, `id`, … |
| time | `TIME` | one of `name.predictors` | the primary covariate right of `~` | `time`, `t`, … |
| observed value | `DV` | `name.response` | the response left of `~` | `conc`, `dv`, … |
| amount of a dose | `AMT`, on dose rows; `NA` on observation rows | a predictor column (the dose), no event type | a column (`Dose` in `Theoph`), no event type | `dose`, `amt`, `amount` (one value per subject) |
| event type | `EVID`: 0 observation, 1 dose, 2 other, 3 reset, 4 reset and dose | none (only a missing-value column) | none | none |
| compartment | `CMT` | none for compartments; `name.ytype` marks the type of response | none | none |
| rate, duration | `RATE`, `DUR` (duration = amount / rate; special negative codes for rate or duration to be modelled) | none | none | none |
| additional doses, steady state | `II`, `ADDL`, `SS` | none | none | none |
| missing observation | `MDV` | `name.mdv` | `NA` in the response (`na.action`) | empty cell |
| censoring | `CENS` (0 uncensored, 1 left, −1 right) and `LIMIT` | `name.cens` | none | `BLQ` text in the value cell |
| covariates | any further column; constant per subject through `iCov`, time-varying by carrying the last value forward (default), the next backward, or linear interpolation | `name.covariates` (declared) | `outer` covariates are constant within the group, `inner` ones can change | `other` columns |
| occasion | none in the table | `name.occ` | a second nesting level | none |
| units | none in the table | `units` (for x, y and covariates) | `units` and `labels` attributes of the grouped data | in the header, `Name (unit)` |

Sources: S-41 (event types, event table, covariates, fetched summaries), S-42 (`theo_sd`: columns `ID`, `TIME`, `DV`, `AMT`, `EVID`, `CMT`, `WT`; summary), S-43 (`CENS` and `LIMIT`, summary), S-44 (arguments of `saemixData`, summaries), S-39 (`groupedData`, `nlsList`: read in full), S-40 (`Theoph`: `Subject`, `Wt` in kg, `Dose` in mg/kg, `Time` in hr, `conc` in mg/L; read in full). Whether `rxode2` reads the column names case-insensitively was not confirmed from a page that was read.

## 2. Layouts and columns

### DAT-LAY-01 Two layouts, one table
A dataset is a table of records, one per row, with a subject column. There are two layouts. The **profile layout** is today's: no event column; every row is an observation; an optional dose column holds the dose of the subject. The **event layout** has an event column (DAT-REC-01); each row is an observation or a dose (DAT-TYP-01). The layout is decided by the presence of that column and by nothing else, so a file that loads today never changes layout. A profile-layout table is read as the event table of DAT-LEG-01 by the analyses that need doses.

**Exception found by the review of T-150 (finding 6, 2026-11-07).** The presence of a column recognised as `event` (the keys of DAT-REC-01: `event`, `evt`, `evid`, `eventtype`) decides the layout, whatever the cells hold. A legacy file with a text column named like one of these, which is not an event type, is therefore read as the event layout and its rows are refused. The orchestrator's decision on the review was that "presence decides" stays today, because no legacy file in the 30 pinned inputs shows the problem; the case is the open item OD-08.
- Status: `assumed`
- Sources: design choice; S-41 (the event table as the shared format of `rxode2` and `nlmixr2`, summary).

### DAT-COL-01 The columns
The roles of a column, with the layouts that use them:

| role | meaning | type | profile layout | event layout |
|---|---|---|---|---|
| `subject` | identifier of the subject | text or number, compared as written | optional (one subject if absent) | required when there is more than one subject |
| `time` | time of the record, in the unit of the header | number | required | required |
| `concentration` (`conc`) | observed value | number, or text for BLQ (DAT-CEN-02) | required | required on observation rows |
| `dose` (`amt`) | amount of a dose | number > 0 | optional: the dose of the subject, repeated on its rows | required on dose rows, absent on observation rows |
| `event` | record type (DAT-TYP-01) | number or text | absent | required |
| `rate`, `dur` | rate or duration of an infusion | number ≥ 0 | absent | optional, dose rows only |
| `cmt` | compartment (DAT-CMT-01) | number or text | absent | optional |
| `mdv` | observation not to be used (DAT-CEN-01) | 0 or 1 | absent | optional |
| `cens`, `limit` | censoring (DAT-CEN-02) | −1, 0, 1; number | absent | optional |
| `route` | route text, as today | text | optional | optional |
| covariate | a declared covariate (DAT-COV-01) | number or text | optional | optional |
| `other` | anything else, shown and carried, not used | any | yes | yes |

**Role ids** (T-150 and the oracle, 2026-11-07). The stored ids stay those of today, `concentration` and `dose`. In this file `conc` and `amt` are the short names of these two roles, and `conc` and `amt` are header names recognised for them (section 0 and DAT-REC-01). The oracle files write the roles as `conc` and `amt`, and `oracle_data.rs` accepts either spelling for the same role. `ColumnRole::ALL` in `crates/caladrius-project/src/worksheet.rs` holds the 14 ids (`time`, `concentration`, `subject`, `dose`, `route`, `event`, `rate`, `dur`, `cmt`, `mdv`, `cens`, `limit`, `covariate`, `other`), and the engine's schema is built from it.

At most one column has each role (as today: the first keeps it, the others become `other` with a note). The time, concentration and dose columns must be numeric, or the role is refused with the note that today's importer gives.
- Status: `assumed`
- Sources: S-41, S-42, S-43, S-44 (the column set, summaries); the repo's importer (today's roles and the "at most one" rule); design choice for the table.

### DAT-REC-01 Recognition: which names are accepted
A column is recognised by its **key**, the lower-case letters and digits of its name with spaces, underscores, dots and a trailing unit removed (as today). Upper and lower case make no difference, so `ID`, `TIME`, `DV`, `AMT`, `EVID`, `CMT`, `RATE`, `DUR`, `MDV`, `CENS`, `LIMIT` of an `nlmixr2` file are read. The keys accepted for each role are those of today (subject, time, conc, dose and route, listed in section 0) and these added ones:

| role | added keys |
|---|---|
| `event` | `evid`, `event`, `eventtype`, `evt` |
| `cmt` | `cmt`, `compartment`, `cmpt` |
| `rate` | `rate`, `infusionrate` |
| `dur` | `dur`, `duration`, `tinf` |
| `mdv` | `mdv`, `missing` |
| `cens` | `cens`, `censored`, `censoring` |
| `limit` | `limit`, `lloq`, `loq` |

A column whose name matches nothing keeps the role `other`. A column that matches a role but whose content does not fit it (text in `time`, numbers in `route`, a `mdv` that is not 0 or 1) is left `other` with a note, as today. The role of a column can always be set by the user, and the interface shows the guess with the reason. No guess is silent: the notes list every column that was given a role and every one that was not.
- Status: `assumed`
- Sources: the repo's importer (the key rule and today's names); S-41, S-42, S-43, S-44 (the names used by the open tools, summaries); design choice for the added aliases.

### DAT-REC-02 Names that do not map: occasions, additional doses, steady state
A column recognised as `occ`/`occasion`, `ii`, `addl`, `ss` or `ytype`/`dvid` (the names `saemix` and `rxode2` give to occasions, interdose intervals, additional doses, steady-state flags and response types) is not given a role in this version: it is left `other`, with a note that says the information is not used ("column `addl` holds additional doses; additional doses are not used yet, so each dose row is one dose"). **If such a column contains a non-empty, non-zero value on a dose row, the table is refused** for that subject with the sentence of DAT-ERR-06, because ignoring it would change the dose history silently.
- Status: `assumed`
- Sources: S-41 (`II`, `ADDL`, `SS`, summary), S-44 (`name.occ`, `name.ytype`, summary); design choice (refuse rather than ignore).

## 3. Records

### DAT-TYP-01 Event types
In the event layout the `event` column holds, for each row, one of:

| value | meaning | now |
|---|---|---|
| `0`, `obs`, `observation` | an observation | accepted |
| `1`, `dose` | a dose | accepted |
| `2` ("other"), `3` (reset), `4` (reset and dose) | the codes `rxode2` also defines | refused: `unsupported_event` (DAT-ERR-06) |
| anything else | | refused: `unknown_event` (DAT-ERR-06) |

Text values are compared in lower case. A missing event cell is an error (`unknown_event`), never a default of 0 or 1.
- Status: `documented, untested`
- Sources: S-41 (the five event codes and their meaning, from the fetched event-types page summary; codes 2 to 4 only as listed there); design choice for the text spellings and for refusing 2 to 4.

### DAT-TYP-02 Dose records
A dose row has an amount > 0 (an amount ≤ 0 is `non_positive_amount`, DAT-ERR-06; a missing one is `dose_without_amount`, DAT-ERR-02), a time, and optionally `rate` or `dur`, `cmt`, `route`. The input is a **bolus** (instantaneous) when neither rate nor duration is given, or when both are 0 or empty. It is an **infusion** when one of them is > 0: duration = amount / rate, or rate = amount / duration, so giving either is enough. If both are given and |rate·dur − amount| exceeds 1e-6 of the amount, the row is refused (`rate_duration_mismatch`, DAT-ERR-06). Negative rate or duration (the `rxode2` codes for "modelled rate" and "modelled duration") are refused as `unsupported_event`. A rate or duration derived from the amount and the other one that is not a finite number (an enormous amount, a tiny rate or duration) is refused as `rate_duration_mismatch`, with a sentence that gives the amount, the value given and the value derived (T-150 fix round, 2026-11-07; the card chose this existing code). A subject may have several dose rows in the table (the table can hold them); the analyses decide what to do with more than one (DAT-USE-01, DAT-USE-02).
- Status: `documented, untested` for the relation duration = amount/rate and the bolus/infusion distinction; `assumed` for the mismatch tolerance
- Sources: S-41 (rate, duration and the special negative values, summary); `specs/models.md` MOD-IVI-01 (the infusion model, rate R0 = D/T); design choice for the tolerance.

### DAT-TYP-03 Observation records
An observation row has a time and a value in `conc` (or a missing value, BLQ or censoring mark, DAT-CEN-*), and **no amount**: the `amt` cell must be empty on an observation row of the event layout. `rate`, `dur` and `route` cells on an observation row are ignored with a note. Covariates are read from the row but are one value per subject (DAT-COV-01). (In the profile layout there is no event column and the dose column, repeated on observation rows, is the dose of the subject: DAT-LEG-01.) The notes for the `rate`, `dur` and `route` cells are implemented since the T-150 fix round (a48449a, c8b4955, 2026-11-07); the first version of the code ignored the cells silently (T-150 review, finding 4).
- Status: `assumed`
- Sources: S-41 (observation rows leave the amount missing, summary); design choice.

### DAT-CMT-01 Compartment
The `cmt` column is recognised and kept. In this version the model decides where a dose goes (the depot for first-order absorption, the central compartment otherwise, from the model id and `route`) and where the observation is made (the central compartment), so the values of `cmt` are not interpreted. Rule: the cells must be empty or hold a single value across all the observation rows of the table, or hold the same value as on the dose rows; a table whose observation rows carry **two or more different** `cmt` values (several endpoints, for example a drug and a metabolite) is refused with `several_endpoints`: "the observation rows of subject {S} are in several compartments ({v1} and {v2}); one observed compartment is analysed at a time. Split the table by compartment, or give a table with one." A numbered compartment is not checked against the model.
- Status: `assumed`
- Sources: S-41 (`CMT`, summary), S-44 (`name.ytype`, response types, summary); design choice (one endpoint now, several later: DAT-LAT-01).

## 4. Units, order, errors

### DAT-UNI-01 Units in the header
As today. A header may end in a unit, in round or square brackets: `Time (h)`, `Conc [ng/mL]`, `Amount (mg)`. The unit text is kept as written, attached to the column, and shown with it; it applies to every cell of the column. It is accepted on `time`, `conc`, `amt`, `rate`, `dur`, `limit` and on covariates. The unit of `rate` should be amount per time and the unit of `dur` a time unit; the table is not refused if they are not (units are text), but the results quote them. No unit conversion happens in the table. An event table whose `amt` column has no unit is accepted with a note ("the unit of the amount is not given"). A unit that differs between two columns that should agree (the time unit of `time` and of `dur`) gives a note, not an error.
- Status: `assumed`
- Sources: the repo's importer (the header rule); S-44 (`units` of `saemixData`, summary), S-39 (`groupedData` `units` attribute, read in full: units are metadata attached to the columns); design choice.

### DAT-ORD-01 Order and sorting
Within one subject, the rows must be in non-decreasing time. Rows of different subjects may be interleaved (the check groups the rows by subject label, as today). A dose and an observation at the same time are allowed, in either order of the rows (the dose is taken as occurring first). Two observations at the same time for one subject are the error `duplicate_times` of today. **Scope of the check** (T-150, 2026-11-07, as the T-148 decisions ask): in the event layout only the observation rows of one subject are compared, so a dose and an observation at the same time are not a duplicate, and neither are two dose rows; in the profile layout every row is an observation and the check is unchanged. **The table is never re-sorted silently**: a table whose rows are not in time order is reported with `time_not_increasing` (DAT-ERR-01) and the interface offers the sorted reading as an explicit choice that the user accepts, recorded in the history. Missing times: a row with a missing time is refused (`missing_time`).
- Status: `assumed`
- Sources: the repo's importer (`time_not_increasing`, `duplicate_times`); `AGENTS.md` golden rule 6 (no silent correction); design choice for same-time dose and observation.

### The five errors the card asks for

Each error is a **check on a reading** (the same mechanism as `time_not_increasing` today): the table is imported and shown, the check lists the problem with the first row and the count of the others, and **an analysis of an affected subject is refused** with the same sentence while the other subjects run (results per subject, an error is a result). The text names the row (counted from the first data row, 1-based), the subject and the column, and says what to do. The words in braces are filled in.

### DAT-ERR-01 Time going backwards within a subject
Code `time_not_increasing` (exists today). Sentence, as today: "time goes back at row {r} of subject {S} ({t2} after {t1}); a profile is listed in time order". The interface adds: "sort the rows by subject and time, or ask for the sorted reading".
- Status: `documented, untested` (today's text is in the importer; it is not under a test in this card's scope)
- Sources: the repo's importer; S-39 (grouped data are ordered within the group, summary level); design choice.

### DAT-ERR-02 A dose without an amount
Code `dose_without_amount`. A dose row (event 1, or in the profile layout never) whose amount cell is empty or not a number. Sentence: "row {r} of subject {S} is a dose but has no amount; give the amount in column `{amt}`, or change the event type if it is an observation". When the amount column is missing, or holds text, the code gives two other sentences that say so (T-150 fix round, finding 7: the first version named a column `amt` that did not exist): "... but the table has no amount column; add a column named amt (or amount) with the amount of each dose, or change the event type if it is an observation" and "... but column `{name}` holds text, not numbers, so there is no amount; make the amounts numbers, or change the event type if it is an observation". The oracle checks the pieces "row {r} of subject {S}", "is a dose but has no amount" and the column name.
- Status: `assumed`
- Sources: S-41 (a dose row needs `AMT`, summary); design choice (sentence); card T-150 (the review, finding 7, and the fix round, 2026-11-07).

### DAT-ERR-03 An observation with an amount
Code `amount_on_observation`. An observation row (event 0) of the event layout whose amount cell holds a number. Sentence: "row {r} of subject {S} is an observation but has an amount ({x}); an amount belongs on a dose row. Remove the amount, or change the event type to dose". Value 0 counts as an amount; an empty cell and the markers for a missing value do not.
- Status: `assumed`
- Sources: S-41 (observation rows leave the amount missing, summary); design choice.

### DAT-ERR-04 A subject with no observation
Code `subject_without_observation`. A subject that appears in the table (it has at least one row) and has no observation row in the event layout; or, in the profile layout, whose rows all have a missing time or value. Sentence: "subject {S} has no observation (only {n} dose rows), so there is nothing to analyse or fit for it; add its observations or remove its rows". A subject whose observations are all missing or all BLQ has observation rows, so this check passes; the analysis then says "no usable observation" with the counts, as today.

**Layout scope** (T-150, "Departures from `specs/data.md`", 2026-11-07): the check is applied in the event layout only. In the profile layout it is not applied, so a profile-layout file reads as before the brick (DAT-LEG-02: legacy wins); the clause above for the profile layout is therefore not in force. The same scope holds for the other refusals of DAT-ERR-02 to 06 that come from the event layout (DAT-ERR-06); the profile layout gets only `different_doses` (DAT-LEG-01) and `covariate_varies` (DAT-ERR-05).
- Status: `assumed`
- Sources: design choice; S-41 (a subject is the set of rows with one ID, summary); card T-150 (2026-11-07).

### DAT-ERR-05 A covariate that changes within a subject
Code `covariate_varies`. For each **declared** covariate (DAT-COV-01) the non-missing values of a subject must all be equal (numbers equal as numbers, text as written). Sentence: "covariate `{name}` takes several values for subject {S} ({v1} at row {r1} and {v2} at row {r2}); in this version a covariate has one value per subject. Give one value per subject, or declare the column as not a covariate" (time-varying covariates are not available yet: DAT-COV-02). A column that is not declared as a covariate is never checked. Missing cells are ignored in the check; a subject whose covariate cells are all missing has that covariate missing (NC), not an error.
- Status: `assumed`
- Sources: S-39 (`groupedData`: an outer covariate is constant within the group, an inner one can change, read in full); S-41 (constant covariates through `iCov`, summary); design choice (the check and the sentence).

### DAT-ERR-06 The other refusals
The same mechanism, with these codes and the sentences' content: `unknown_event` (the event cell is empty or not one of DAT-TYP-01), `unsupported_event` (event 2, 3 or 4, or a negative rate or duration: "row {r} of subject {S} has event type {v} (`{meaning}`), which is not supported in this version; supported types are 0 (observation) and 1 (dose)"), `non_positive_amount` (amount ≤ 0), `rate_duration_mismatch` (DAT-TYP-02), `missing_time`, `empty_subject` (a blank subject cell when the table has several subjects: "row {r} has no subject; every row of a table with several subjects needs one"), `unsupported_history` (a non-empty, non-zero value in a column named `ii`, `addl` or `ss` on a dose row: "row {r} of subject {S} asks for additional doses or a steady state (`{column}`), which are not supported in this version; give each dose on its own row"), `several_endpoints` (DAT-CMT-01), `unsupported_censoring` (DAT-CEN-02) and `different_doses` (the existing sentence of the profile layout, DAT-LEG-01).

**Layout scope.** All of these except `different_doses` are applied in the event layout only (DAT-ERR-04). In particular, in a profile-layout file with several subjects a row whose subject cell is blank is dropped silently from the subjects, as `Worksheet::profile` does today (T-150 review, finding 10): `empty_subject` is not raised there. Refusing it would change the reading of a legacy file; the case is the open item OD-09.
- Status: `assumed`
- Sources: design choice; S-41 (the values that are not supported are those the sources define: event types 2 to 4, negative rate and duration, `II`, `ADDL`, `SS`, summaries).

### DAT-ERR-07 A refused table keeps the records of the other subjects
A refusal belongs to the subjects it names. The reading of a table with a refusal still holds the dataset of every subject, the unaffected ones complete (their doses, observations and counts), and the refusal code is the first entry of the reading's `checks`, with a sentence that holds the pieces OD-02 lists. The oracle fixes this with the `unaffected_records` of every `refuse_*.json` file (OD-02): the records of the subjects the problem does not touch must be those of the same table without the problem.
- Status: `confirmed by oracle`
- Oracle: `oracle/expected/data/refuse_*.json` (the fields `error`, `unaffected` and `unaffected_records`), checked by `crates/caladrius-project/tests/oracle_data.rs`: `unaffected_case` for the records of the unaffected subjects (25 tests, T-150 thread) and `refusal_case` for the refusal code as first check and the words of its sentence.
- Sources: card T-148 (the fix round, 654c3cc: the refusal code is required first in `checks`, 2026-11-07); card T-150; DAT-ERR-01 above (the mechanism of a check on a reading).

### DAT-ERR-08 An affected subject is marked and refused in an analysis
In the dataset, each subject that a problem touches carries `refusal`, the code of its first problem (absent when nothing is wrong). An analysis of an affected subject is to be refused with the sentence of the check, while the analyses of the other subjects run (results per subject, an error is a result). Two limits: (1) only the marking is built; the routing of the dataset into the analyses is DAT-USE-02, a later card, and no analysis reads `refusal` yet; (2) in the profile layout `Worksheet::profile()` refuses the subject itself, with its own sentences (`dose_varies`, `missing_time`, ...), which can differ from the check's (DAT-LEG-01, DAT-LEG-02). A problem that names no subject (the blank subject of `empty_subject`) marks none. The oracle does not test the field `refusal` (the oracle subject of `empty_subject` is none) nor the refusal of an analysis.
- Status: `assumed`
- Sources: card T-150 (the field `refusal` of `SubjectRecords`, `dataset.rs`); DAT-ERR-01 above; design choice for the refusal of an analysis.

## 5. Covariates, missing values, BLQ and censoring

### DAT-COV-01 Covariates: declared, constant per subject
A covariate is a column that the user **declares** (role `covariate`, a list in the import dialog or the command); no column becomes a covariate by its name alone (the interface may suggest `wt`, `weight`, `age`, `sex`, `bsa`, `crcl`, `height` as candidates). A declared covariate is numeric or categorical (text); it has one value per subject (DAT-ERR-05); the value of a subject is its non-missing value. A declared name is matched to a header **without regard to case**, like the roles (DAT-REC-01); the first version matched the case exactly (T-150 review, finding 8; fixed in the fix round, 2026-11-07). The covariates are shown in a subject table (one row per subject) next to the profiles, and carried unchanged into every per-subject result. Neither NCA nor the individual fit uses them now (DAT-USE-03); they are the input of the population brick.
- Status: `assumed`
- Sources: S-44 (`name.covariates` is a declared list, summary); S-39 (outer covariates, read in full); S-40 (`Theoph`: `Wt` and `Dose` are per-subject values repeated on every row); design choice.

### DAT-COV-02 Time-varying covariates: later
Covariates that change within a subject (a weight measured at each visit, a creatinine clearance) are **not accepted now**: the table is refused with `covariate_varies` if the column is declared, and an undeclared varying column is carried as `other`. The interpolation conventions of `rxode2` (carry the last value forward, the default; carry the next value backward; linear interpolation) are recorded for the card that adds them.
- Status: `later`
- Sources: S-41 (covariate interpolation methods, summary); S-39 (inner covariates).

### DAT-CEN-01 Missing values
A missing observation is an empty cell, or a marker the importer already treats as missing, or a row with `mdv` = 1. An observation row with a missing value (or `mdv` = 1) stays in the table with the reason `missing`, is counted as `n_missing` and is not used by any analysis (FIT-DAT-01). A dose row is not affected by `mdv`. A `mdv` value other than 0, 1 or empty is refused (`unknown_mdv`).
- Status: `documented, untested` for the meaning of the flag; `assumed` for the refusal of other values
- Sources: S-41 (`MDV`, summary); S-44 (`name.mdv`, summary); `specs/fit.md` FIT-DAT-01 (`n_missing`).

### DAT-CEN-02 BLQ and censoring columns
Two ways to mark a value below the limit of quantification are accepted, and they mean the same:
(a) **text in the value cell**, as today (`BLQ`, `<LOQ`, `< 0.5` …): the row is BLQ, the stored value is 0 with the BLQ flag in the worksheet (NCA-DAT-05c); a numeric limit written after `<` is kept as the limit of the row;
(b) **a `cens` column** with 1 (left-censored: the true value is below the limit): the value cell then holds the **limit** (the convention of `rxode2`, where DV carries the limit on a censored row), and the row is read as BLQ with that limit; the `limit` column, when present, is the other (lower) bound of the censoring interval, kept and unused now. `cens` = 0 or empty is an ordinary observation.

**What the dataset keeps** (T-150, the review, finding 5, and its `limit` correction, 019a2d1, 2026-11-07; the orchestrator's correction of its own instruction is in the thread). An observation record has a `status` (`observed`, `missing` or `blq`), a `value`, and two optional numbers. For a BLQ row **`value` is null in the dataset** (the value is not kept; the worksheet still stores 0 with the flag, as above): the oracle expects null (`event_censoring.json`), and this settles the question of 0 against null, which the T-150 review left to this card. `limit` is the number after the `<` of the cell, and otherwise, on a row with `cens` = 1, the number in the value cell. `lower_limit` is the `limit` column of a row with `cens` = 1, kept and unused; both are left out of the JSON when absent. Two edge cases are decided by the code, and the spec states them as follows:
(i) a cell `<x` on a row with `cens` = 1: the number in the cell is the limit, as for any `<x` cell;
(ii) the cell is the word `BLQ` (no number) on a row with `cens` = 1: the row is **not refused** and has **no limit** (`limit` absent, not 0). This departs from the sentence above that refuses a censored row with a missing limit, which holds for an empty value cell only. Whether such a row should be refused is the open item OD-10. `cens` = −1 (right-censored), or any other value, is refused (`unsupported_censoring`: "row {r} of subject {S} is marked right-censored (or has an unknown censoring code {v}); only left-censored values (1) are supported"). A censored row with `cens` = 1 and a missing limit is refused (`unsupported_censoring`, "the limit is missing").
In this version every BLQ row is **left out** of the analyses and counted (`n_blq`), by NCA according to its BLQ rules (`specs/nca.md` NCA-DAT-05) and by the fit as in FIT-DAT-01 and FIT-ML-19. The censored-likelihood method that would use the limit is brick 3 (`specs/fit.md` OF-21). The columns are specified now so that a table does not change when it arrives.
- Status: `documented, untested` for the `cens` and `limit` semantics; `assumed` for the mapping onto the BLQ flag, and for the two edge cases (the code does it, the oracle does not test it)
- Sources: S-43 (`CENS` and `LIMIT`, the old `nlmixr` censoring vignette and the `rxode2` page, summaries); S-44 (`name.cens`, summary); S-46 (BLQ methods, abstract summary); `specs/nca.md` NCA-DAT-05c.

## 6. Today's file in the new table

### DAT-LEG-01 The profile layout is the event table of a single dose at time 0
A profile-layout table maps into the event table as follows, per subject: one dose record at time 0 with amount = the value of the dose column (when the column exists and holds a value; a bolus, or an infusion when the user's model option says so), followed by every row as an observation record with its time, value, BLQ flag and missing flag. The dose column holds **one value per subject**: two different values in one subject stay the existing error ("subject {S} has two different doses ({a} and {b}) in column `{name}`; one dose per profile"), and a subject without a dose value has no dose record (a fit then asks for the dose as today). **Two levels** (T-150, 2026-11-07): the reading of the table now carries the check `different_doses` with the existing sentence and marks the subject, and `Worksheet::profile()` still raises `dose_varies` when the subject is analysed. The two are one rule at two levels and nothing was renamed. The check uses the same comparison as `dose_varies` (`first != amount`, exact, blank-subject rows skipped), so it fires on the subjects for which `profile()` would raise `dose_varies`; a unit test of `caladrius-project` compares the two rules on the same tables. `profile()` can refuse the same subject earlier for another reason (a row with no time, `missing_time`: DAT-LEG-02). **This is a deliberate change** (T-150 review, major 1, accepted by the orchestrator): a legacy file with two doses in one subject could never be analysed, and now says so at import. Its reading carries one more check, the interface asks for an acknowledgement, and the order of the readings does not move (the ranking of a profile file uses its time checks only). It is the only way the checks of a profile-layout file differ from those before the brick; `legacy_regression` names it as its one allowed difference. The mapping is a view for the analyses that need doses; **the worksheet, the file, the column roles, the notes and the stored form of a project are not changed by it.**
- Status: `assumed`
- Sources: the repo's worksheet (`profile()` and the one-dose rule); design choice.

### DAT-LEG-02 What must not change
Every file that loads today loads with the same columns, roles, notes and checks, in the same order, and gives the same result in every analysis: a dataset opened in the previous version gives the same NCA table, the same fit (estimates, statistics, trace), the same history entry and the same exported file after the brick as before it. This covers at least `oracle/data/theoph.csv`, `indometh.csv`, `edge_*.csv`, `synthetic_lz.csv`, the CSV and JSON of the fit oracle, and any project saved before the brick. A file is read as the event layout only if it contains a column recognised as `event`; the added keys of DAT-REC-01 (`cmt`, `rate`, `dur`, `mdv`, `cens`, `limit`) may be recognised in a profile-layout file that happens to contain such a column, with a note, and are not used by the analyses there. If recognising one of the added keys would change the result of a legacy file (a column called `limit` or `rate` in an existing file that today is `other`), the legacy behaviour wins: such a column stays `other` in the profile layout. **As built** (T-150, "Departures from `specs/data.md`", 2026-11-07): the added keys are not recognised at all in a profile-layout file, with no note. In `oracle/data/population/profile_limit_rate.csv` the columns `limit`, `rate` and `mdv` keep the role `other`, and the oracle expects exactly that (`oracle/expected/data/profile_limit_rate.json`, rule DAT-LEG-02). This is the rule; OD-07 (1) is settled by it. The regression test is a byte-level comparison of worksheets, notes, checks, NCA outputs and fit outputs between the last version before the brick and the new one on all the files above (OD-03). The test exists: `crates/caladrius-project/tests/legacy_regression.rs` compares readings, profiles and saved projects with pins made from the code of 5815119, byte for byte, over 30 inputs (the 27 of the card and the 3 legacy-style inputs of the T-150 fix round: backwards times, duplicate times, two doses in one subject); its one allowed difference is the check `different_doses` (DAT-LEG-01).

**The order of two refusals is not part of the contract.** When a row with no time comes before a conflicting dose in a profile-layout file, `Worksheet::profile()` raises `missing_time` first, while the dataset raises `different_doses` and not `missing_time` (T-150 confirmation review, minor, 2026-11-07). Both refuse the subject. The spec fixes the code and the sentence of the refusal only when one problem is present, and the code of the first check (DAT-ERR-07); which of two problems is reported first is not specified and no test depends on it.
- Status: `assumed`
- Sources: card T-147 (the requirement); `AGENTS.md` golden rules 3 and 4 (the same input gives the same output); design choice for the legacy-wins clause.

## 7. What the analyses do with the table

### DAT-USE-01 NCA
Today: `subject = none` analyses every subject, one analysis per subject, results and errors per subject. After the brick: the same, for both layouts. In the event layout the analysis of a subject uses its observation rows (BLQ and missing rows handled by the NCA rules) and its first dose row (amount, time, duration for an infusion from `dur` or `rate`, route from `route` if given); a subject with **more than one dose row** is refused for that subject only: "subject {S} has {n} dose rows; the NCA of this version analyses a single dose per profile". If the user's explicit option gives an infusion duration and the file gives another, the option is used and a note says so. A dose time other than 0 shifts the time axis of the analysis (the time of the profile is measured from the dose); an observation before the dose is left out with the reason `before_dose` (FIT-ERR-01).
- Status: `assumed`
- Sources: the repo's NCA specification and worksheet (today's behaviour); `specs/nca.md` NCA-DAT-*; design choice for the new parts.

### DAT-USE-02 Individual fit
Today: a fit needs one subject; with several subjects and none named it is refused (`subject_required`). After the brick the default is **unchanged** (so no existing behaviour moves), and one value is added: `subject = all`, which runs **one independent fit per subject** with the same model and options (the initial estimates are generated per subject, FIT-INI-01), and returns a list of per-subject results; an error in one subject is that subject's result and does not stop the others. No parameter is shared and nothing is pooled: sharing parameters across subjects is the population brick. The fit takes the dose and its time and duration from the subject's dose row as in DAT-USE-01; more than one dose row is refused for that subject. The objective is least squares or maximum likelihood (`specs/fit.md` FIT-ML-01), chosen once for all.
- Status: `assumed`
- Sources: the repo's worksheet and fit specification (today's behaviour, `subject_required`); `specs/fit.md` FIT-ML-01; design choice for `subject = all`.

### DAT-USE-03 What the covariates do now
Nothing in the analyses: NCA and the individual fit ignore the declared covariates. They are listed in the subject table, copied into each per-subject result, and recorded in the history of the dataset. Using them is the population brick.
- Status: `assumed`
- Sources: design choice; card T-139 (population modelling is the next brick).

## 8. Vocabulary

### DAT-VOC-01 Ids
Roles (stored ids: `subject`, `time`, `concentration` (short name `conc`), `dose` (short name `amt`), `event`, `rate`, `dur`, `cmt`, `mdv`, `cens`, `limit`, `route`, `covariate`, `other`); event values (`obs`, `dose`; codes 0 and 1); check codes (`time_not_increasing`, `duplicate_times`, `dose_without_amount`, `amount_on_observation`, `subject_without_observation`, `covariate_varies`, `unknown_event`, `unsupported_event`, `non_positive_amount`, `rate_duration_mismatch`, `missing_time`, `empty_subject`, `unsupported_history`, `several_endpoints`, `unsupported_censoring`, `unknown_mdv`, `different_doses`); options (`covariates` = list of declared columns; `subject` = a label, none, or `all`). Role ids follow the style of `specs/nca.md` section 10 (lower case, underscores).
- Status: `assumed`
- Sources: the repo's role ids; `specs/nca.md` section 10 (naming style); design choice.

## 9. Later

### DAT-LAT-01 Described by the sources, outside the scope now
Listed with their sources; no rule, no test. (1) Event types 2, 3 and 4; additional doses and steady state (`II`, `ADDL`, `SS`); modelled rate and duration (negative codes): S-41. (2) Time-varying covariates and their interpolation: S-41, S-39. (3) Occasions (several occasions per subject): S-44 (`name.occ`). (4) Several observation endpoints and response types: S-44 (`name.ytype`), S-41 (`CMT`). (5) Right and interval censoring and the censored likelihood (M3, M4): S-43, S-46. (6) Unit conversion between the amount and the concentration units. (7) A sorting option applied at import.
- Status: `later`
- Sources: S-39, S-41, S-43, S-44, S-46 (summaries, as said in each).

## 10. Worked example

(No rule.) A table in the event layout with two subjects, semicolon-separated, decimal point, as it should load (`ID;TIME;DV;AMT;EVID;WT`):

```
ID;TIME;DV;AMT;EVID;WT
1;0;;100;1;70
1;0.5;3.1;;0;70
1;2;5.8;;0;70
1;8;2.0;;0;70
2;0;;120;1;82
2;0.5;2.7;;0;82
2;2;5.2;;0;82
2;8;<0.5;;0;82
```
Roles: `ID` subject, `TIME` time, `DV` conc, `AMT` amt, `EVID` event, `WT` other until declared a covariate. Subject 1: one dose of 100 at time 0 and three observations; subject 2: one dose of 120, two observations and one BLQ (stored 0, flag, limit 0.5, `n_blq` = 1). The subject table: ID 1 (WT 70), ID 2 (WT 82). With `WT` declared, the check passes. If the last WT of subject 2 were 80, the check says: "covariate `WT` takes several values for subject 2 (82 at row 5 and 80 at row 8); in this version a covariate has one value per subject. Give one value per subject, or declare the column as not a covariate" (rows counted from the first data row). If row 3 read `1;0.5;3.1;5;0;70`, the check says: "row 3 of subject 1 is an observation but has an amount (5); an amount belongs on a dose row. Remove the amount, or change the event type to dose".
The same data in the profile layout (today): `ID;TIME;DV;DOSE` with the dose repeated on every row of a subject, no event column; it loads as now and maps to the same event table (DAT-LEG-01).

## 11. Not covered here

The model of the population itself (random effects, parameter distributions, estimation algorithms) is brick 3. This file does not specify file formats other than CSV (the Excel and clipboard readers of the importer keep their present rules).

## 12. Open items for the oracle (T-148)

| id | item | rule | who and how |
|---|---|---|---|
| OD-01 | **Files that must load**, to be written as small CSV files under `oracle/data/` with the expected worksheet (roles, notes, subject list, per-subject dose and observation counts, `n_missing`, `n_blq`) in a JSON file: (a) the worked example of section 10, with `ID;TIME;DV;AMT;EVID;WT`, upper-case names; (b) the same data with lower-case `subject,time,conc,amt,event`, with `event` as text `obs`/`dose`; (c) `Theoph` exported from R (`Subject, Wt, Dose, Time, conc` of the `datasets` package, 12 subjects, 132 rows, no event column: profile layout, `Dose` the per-subject dose, `Wt` carried as `other`, and then declared a covariate) and `Indometh` (existing file); (d) an infusion table with `RATE`, one with `DUR`, one with both consistent; (e) a censoring table with `CENS` = 1 and the limit in `DV`; (f) a semicolon/decimal-comma version of (a); (g) interleaved subjects (rows of subject 2 between rows of subject 1, each in time order). | DAT-LAY-01, COL-01, REC-01, TYP-*, CEN-* | T-148 |
| OD-02 | **Files that must be refused**, each with the code and the sentence pieces the test checks (code, row number, subject, column name): time going backwards (`time_not_increasing`); a dose row without amount (`dose_without_amount`); an observation with an amount (`amount_on_observation`); a subject with a dose and no observation (`subject_without_observation`); a declared covariate that changes within a subject (`covariate_varies`); event type 2, 3, 4 (`unsupported_event`); unknown event text (`unknown_event`); a negative rate (`unsupported_event`); `RATE` and `DUR` inconsistent with `AMT` (`rate_duration_mismatch`); an amount of 0 or negative (`non_positive_amount`); two observation compartments (`several_endpoints`); `CENS` = −1 and `CENS` = 2 (`unsupported_censoring`); a non-empty `ADDL` on a dose row (`unsupported_history`); a blank subject cell in a multi-subject table (`empty_subject`); `MDV` = 2 (`unknown_mdv`); a missing time (`missing_time`). The test compares the code and the named row, subject and column, and that the other subjects of the same table still analyse. | DAT-ERR-01 to 06 | T-148 |
| OD-03 | **Legacy equivalence.** Byte comparison between the last version before the brick (a pinned build) and the new one on `oracle/data/*.csv`, the fit oracle data and a project saved before the brick: worksheets (roles, notes, checks), NCA outputs, fit outputs, history entries, exported files. This is the test of DAT-LEG-02. | DAT-LEG-01, 02 | T-148 and the implementer |
| OD-04 | **Independent reference for the table checks.** The expected subject list, counts, ordering check and covariate constancy of OD-01 are computed in R 4.5.2 from the file with base R `read.csv`, and the constancy and grouping cross-checked with `nlme::groupedData` and `nlme::getGroups` (installed with nlme 3.1.168; `outer` covariates are those constant within groups). The reference decides the data facts, not the sentences. | DAT-COV-01, ERR-05, ORD-01 | T-148 |
| OD-05 | **Per-subject parity.** NCA with `subject = none` on `theoph.csv` gives, for each subject, exactly the result of the analysis of that subject alone (exists as a test for NCA; add the event-layout version of the same data); the fit with `subject = all` on Indometh gives per subject the fit of that subject alone; an error in one subject does not change the others (one subject with a dose of 0 in the event layout). | DAT-USE-01, 02 | T-148 |
| OD-06 | **`rxode2` event tables as a later cross-check.** When `rxode2` can be installed (Q-021: needs Rtools; absent on this PC), build the event tables of OD-01 with `et()` and compare the columns it produces (ID, TIME, AMT, EVID, CMT, RATE/DUR) with the files. Until then the column names come from documentation summaries, flagged `documented, untested`. | DAT-REC-01, TYP-* | later |
| OD-07 | **Open design choices.** (1) The added name keys (`evt`, `tinf`, `lloq`, …) and whether `limit` and `rate` should ever be recognised in the profile layout (DAT-LEG-02): settled for the profile layout by the rule of DAT-LEG-02 (T-151); the added name keys are still open. (2) Whether the table should accept `mdv` on dose rows. (3) Where the same-time order of a dose and an observation matters (the observation at the dose time is a pre-dose sample). (4) A unit-conversion layer (DAT-LAT-01). (5) Whether `subject = all` should be the default for NCA-like batches in the interface only. (6) Case-sensitivity of `rxode2` column names (not confirmed from a read page). | several | orchestrator |
| OD-08 | **A legacy file with a text column named like an event column** (`event`, `evt`, `evid`, `eventtype`) flips to the event layout and is refused (T-150 review, finding 6), against "a file that loads today never changes layout" (DAT-LAY-01). Presence decides today, since no pinned legacy file shows the problem. To decide: refuse to read such a column as `event` when its cells are not event types (keep the profile layout and give it the role `other` with a note), or keep the refusal and say so in the interface. Needs a legacy file that shows it. | DAT-LAY-01, DAT-LEG-02 | Abdou |
| OD-09 | **A blank subject cell in a profile-layout file with several subjects** is dropped silently (T-150 review, finding 10), as `Worksheet::profile` does today. `empty_subject` is raised in the event layout only (DAT-ERR-06). To decide: raise it in the profile layout too, which changes the reading of a legacy file that has such a row, or keep the legacy behaviour and show a note. | DAT-ERR-06, DAT-LEG-02 | Abdou |
| OD-10 | **The word `BLQ` on a row with `cens` = 1** has no number: it is not refused and has no limit, though DAT-CEN-02 refuses a censored row with a missing limit (an empty value cell). To decide: refuse it as `unsupported_censoring` ("the limit is missing"), or accept it as a censored row without a limit and leave brick 3 to refuse it when the censored likelihood needs the limit. A test of the oracle would then need a file (`oracle/data/population/refuse_cens_no_limit.csv` covers the empty cell only). | DAT-CEN-02 | Abdou |
