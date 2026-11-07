# The population dataset (event table): behaviour specification

Card T-147 (brick 2 of iteration 3, T-139), written by the `reader` agent on 2026-11-07, in its own words, before any oracle or code. It specifies the table that holds several subjects, each with its doses, observations, BLQ flags and covariates, and how today's single-subject worksheet maps into it. The maximum-likelihood fit that reads one profile of such a table is in `specs/fit.md` section 13; the NCA is in `specs/nca.md`. Sources are cited by the keys of `specs/sources.md` (section 13 holds the keys S-37 to S-45 added for this file); `D` is a design argument shown here. **Nothing in this file is implemented or tested.**

## 0. How to read this file

**Status tags** (see `specs/README.md`): `documented, untested` (found in an allowed source, not exercised), `assumed` (a design choice of Caladrius, to become a test before it is relied on), `later` (described by a source but outside the current scope: listed, not specified, no test). No rule is `confirmed by oracle`: the dataset oracle is open items OD-01 onwards (section 12), card T-148.

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

Sources: S-39 (event types, event table, covariates, fetched summaries), S-40 (`theo_sd`: columns `ID`, `TIME`, `DV`, `AMT`, `EVID`, `CMT`, `WT`; summary), S-41 (`CENS` and `LIMIT`, summary), S-42 (arguments of `saemixData`, summaries), S-37 (`groupedData`, `nlsList`: read in full), S-38 (`Theoph`: `Subject`, `Wt` in kg, `Dose` in mg/kg, `Time` in hr, `conc` in mg/L; read in full). Whether `rxode2` reads the column names case-insensitively was not confirmed from a page that was read.

## 2. Layouts and columns

### DAT-LAY-01 Two layouts, one table
A dataset is a table of records, one per row, with a subject column. There are two layouts. The **profile layout** is today's: no event column; every row is an observation; an optional dose column holds the dose of the subject. The **event layout** has an event column (DAT-REC-01); each row is an observation or a dose (DAT-TYP-01). The layout is decided by the presence of that column and by nothing else, so a file that loads today never changes layout. A profile-layout table is read as the event table of DAT-LEG-01 by the analyses that need doses.
- Status: `assumed`
- Sources: design choice; S-39 (the event table as the shared format of `rxode2` and `nlmixr2`, summary).

### DAT-COL-01 The columns
The roles of a column, with the layouts that use them:

| role | meaning | type | profile layout | event layout |
|---|---|---|---|---|
| `subject` | identifier of the subject | text or number, compared as written | optional (one subject if absent) | required when there is more than one subject |
| `time` | time of the record, in the unit of the header | number | required | required |
| `conc` | observed value | number, or text for BLQ (DAT-CEN-02) | required | required on observation rows |
| `amt` (alias `dose`) | amount of a dose | number > 0 | optional: the dose of the subject, repeated on its rows | required on dose rows, absent on observation rows |
| `event` | record type (DAT-TYP-01) | number or text | absent | required |
| `rate`, `dur` | rate or duration of an infusion | number ≥ 0 | absent | optional, dose rows only |
| `cmt` | compartment (DAT-CMT-01) | number or text | absent | optional |
| `mdv` | observation not to be used (DAT-CEN-01) | 0 or 1 | absent | optional |
| `cens`, `limit` | censoring (DAT-CEN-02) | −1, 0, 1; number | absent | optional |
| `route` | route text, as today | text | optional | optional |
| covariate | a declared covariate (DAT-COV-01) | number or text | optional | optional |
| `other` | anything else, shown and carried, not used | any | yes | yes |

At most one column has each role (as today: the first keeps it, the others become `other` with a note). The time, concentration and dose columns must be numeric, or the role is refused with the note that today's importer gives.
- Status: `assumed`
- Sources: S-39, S-40, S-41, S-42 (the column set, summaries); the repo's importer (today's roles and the "at most one" rule); design choice for the table.

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
- Sources: the repo's importer (the key rule and today's names); S-39, S-40, S-41, S-42 (the names used by the open tools, summaries); design choice for the added aliases.

### DAT-REC-02 Names that do not map: occasions, additional doses, steady state
A column recognised as `occ`/`occasion`, `ii`, `addl`, `ss` or `ytype`/`dvid` (the names `saemix` and `rxode2` give to occasions, interdose intervals, additional doses, steady-state flags and response types) is not given a role in this version: it is left `other`, with a note that says the information is not used ("column `addl` holds additional doses; additional doses are not used yet, so each dose row is one dose"). **If such a column contains a non-empty, non-zero value on a dose row, the table is refused** for that subject with the sentence of DAT-ERR-06, because ignoring it would change the dose history silently.
- Status: `assumed`
- Sources: S-39 (`II`, `ADDL`, `SS`, summary), S-42 (`name.occ`, `name.ytype`, summary); design choice (refuse rather than ignore).

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
- Sources: S-39 (the five event codes and their meaning, from the fetched event-types page summary; codes 2 to 4 only as listed there); design choice for the text spellings and for refusing 2 to 4.

### DAT-TYP-02 Dose records
A dose row has an amount > 0 (an amount ≤ 0 is `non_positive_amount`, DAT-ERR-06; a missing one is `dose_without_amount`, DAT-ERR-02), a time, and optionally `rate` or `dur`, `cmt`, `route`. The input is a **bolus** (instantaneous) when neither rate nor duration is given, or when both are 0 or empty. It is an **infusion** when one of them is > 0: duration = amount / rate, or rate = amount / duration, so giving either is enough. If both are given and |rate·dur − amount| exceeds 1e-6 of the amount, the row is refused (`rate_duration_mismatch`, DAT-ERR-06). Negative rate or duration (the `rxode2` codes for "modelled rate" and "modelled duration") are refused as `unsupported_event`. A subject may have several dose rows in the table (the table can hold them); the analyses decide what to do with more than one (DAT-USE-01, DAT-USE-02).
- Status: `documented, untested` for the relation duration = amount/rate and the bolus/infusion distinction; `assumed` for the mismatch tolerance
- Sources: S-39 (rate, duration and the special negative values, summary); `specs/models.md` MOD-IVI-01 (the infusion model, rate R0 = D/T); design choice for the tolerance.

### DAT-TYP-03 Observation records
An observation row has a time and a value in `conc` (or a missing value, BLQ or censoring mark, DAT-CEN-*), and **no amount**: the `amt` cell must be empty on an observation row of the event layout. `rate`, `dur` and `route` cells on an observation row are ignored with a note. Covariates are read from the row but are one value per subject (DAT-COV-01). (In the profile layout there is no event column and the dose column, repeated on observation rows, is the dose of the subject: DAT-LEG-01.)
- Status: `assumed`
- Sources: S-39 (observation rows leave the amount missing, summary); design choice.

### DAT-CMT-01 Compartment
The `cmt` column is recognised and kept. In this version the model decides where a dose goes (the depot for first-order absorption, the central compartment otherwise, from the model id and `route`) and where the observation is made (the central compartment), so the values of `cmt` are not interpreted. Rule: the cells must be empty or hold a single value across all the observation rows of the table, or hold the same value as on the dose rows; a table whose observation rows carry **two or more different** `cmt` values (several endpoints, for example a drug and a metabolite) is refused with `several_endpoints`: "the observation rows of subject {S} are in several compartments ({v1} and {v2}); one observed compartment is analysed at a time. Split the table by compartment, or give a table with one." A numbered compartment is not checked against the model.
- Status: `assumed`
- Sources: S-39 (`CMT`, summary), S-42 (`name.ytype`, response types, summary); design choice (one endpoint now, several later: DAT-LAT-01).

## 4. Units, order, errors

### DAT-UNI-01 Units in the header
As today. A header may end in a unit, in round or square brackets: `Time (h)`, `Conc [ng/mL]`, `Amount (mg)`. The unit text is kept as written, attached to the column, and shown with it; it applies to every cell of the column. It is accepted on `time`, `conc`, `amt`, `rate`, `dur`, `limit` and on covariates. The unit of `rate` should be amount per time and the unit of `dur` a time unit; the table is not refused if they are not (units are text), but the results quote them. No unit conversion happens in the table. An event table whose `amt` column has no unit is accepted with a note ("the unit of the amount is not given"). A unit that differs between two columns that should agree (the time unit of `time` and of `dur`) gives a note, not an error.
- Status: `assumed`
- Sources: the repo's importer (the header rule); S-42 (`units` of `saemixData`, summary), S-37 (`groupedData` `units` attribute, read in full: units are metadata attached to the columns); design choice.

### DAT-ORD-01 Order and sorting
Within one subject, the rows must be in non-decreasing time. Rows of different subjects may be interleaved (the check groups the rows by subject label, as today). A dose and an observation at the same time are allowed, in either order of the rows (the dose is taken as occurring first). Two observations at the same time for one subject are the error `duplicate_times` of today. **The table is never re-sorted silently**: a table whose rows are not in time order is reported with `time_not_increasing` (DAT-ERR-01) and the interface offers the sorted reading as an explicit choice that the user accepts, recorded in the history. Missing times: a row with a missing time is refused (`missing_time`).
- Status: `assumed`
- Sources: the repo's importer (`time_not_increasing`, `duplicate_times`); `AGENTS.md` golden rule 6 (no silent correction); design choice for same-time dose and observation.

### The five errors the card asks for

Each error is a **check on a reading** (the same mechanism as `time_not_increasing` today): the table is imported and shown, the check lists the problem with the first row and the count of the others, and **an analysis of an affected subject is refused** with the same sentence while the other subjects run (results per subject, an error is a result). The text names the row (counted from the first data row, 1-based), the subject and the column, and says what to do. The words in braces are filled in.

### DAT-ERR-01 Time going backwards within a subject
Code `time_not_increasing` (exists today). Sentence, as today: "time goes back at row {r} of subject {S} ({t2} after {t1}); a profile is listed in time order". The interface adds: "sort the rows by subject and time, or ask for the sorted reading".
- Status: `documented, untested` (today's text is in the importer; it is not under a test in this card's scope)
- Sources: the repo's importer; S-37 (grouped data are ordered within the group, summary level); design choice.

### DAT-ERR-02 A dose without an amount
Code `dose_without_amount`. A dose row (event 1, or in the profile layout never) whose amount cell is empty or not a number. Sentence: "row {r} of subject {S} is a dose but has no amount; give the amount in column `{amt}`, or change the event type if it is an observation".
- Status: `assumed`
- Sources: S-39 (a dose row needs `AMT`, summary); design choice (sentence).

### DAT-ERR-03 An observation with an amount
Code `amount_on_observation`. An observation row (event 0) of the event layout whose amount cell holds a number. Sentence: "row {r} of subject {S} is an observation but has an amount ({x}); an amount belongs on a dose row. Remove the amount, or change the event type to dose". Value 0 counts as an amount; an empty cell and the markers for a missing value do not.
- Status: `assumed`
- Sources: S-39 (observation rows leave the amount missing, summary); design choice.

### DAT-ERR-04 A subject with no observation
Code `subject_without_observation`. A subject that appears in the table (it has at least one row) and has no observation row in the event layout; or, in the profile layout, whose rows all have a missing time or value. Sentence: "subject {S} has no observation (only {n} dose rows), so there is nothing to analyse or fit for it; add its observations or remove its rows". A subject whose observations are all missing or all BLQ has observation rows, so this check passes; the analysis then says "no usable observation" with the counts, as today.
- Status: `assumed`
- Sources: design choice; S-39 (a subject is the set of rows with one ID, summary).

### DAT-ERR-05 A covariate that changes within a subject
Code `covariate_varies`. For each **declared** covariate (DAT-COV-01) the non-missing values of a subject must all be equal (numbers equal as numbers, text as written). Sentence: "covariate `{name}` takes several values for subject {S} ({v1} at row {r1} and {v2} at row {r2}); in this version a covariate has one value per subject. Give one value per subject, or declare the column as not a covariate" (time-varying covariates are not available yet: DAT-COV-02). A column that is not declared as a covariate is never checked. Missing cells are ignored in the check; a subject whose covariate cells are all missing has that covariate missing (NC), not an error.
- Status: `assumed`
- Sources: S-37 (`groupedData`: an outer covariate is constant within the group, an inner one can change, read in full); S-39 (constant covariates through `iCov`, summary); design choice (the check and the sentence).

### DAT-ERR-06 The other refusals
The same mechanism, with these codes and the sentences' content: `unknown_event` (the event cell is empty or not one of DAT-TYP-01), `unsupported_event` (event 2, 3 or 4, or a negative rate or duration: "row {r} of subject {S} has event type {v} (`{meaning}`), which is not supported in this version; supported types are 0 (observation) and 1 (dose)"), `non_positive_amount` (amount ≤ 0), `rate_duration_mismatch` (DAT-TYP-02), `missing_time`, `empty_subject` (a blank subject cell when the table has several subjects: "row {r} has no subject; every row of a table with several subjects needs one"), `unsupported_history` (a non-empty, non-zero value in a column named `ii`, `addl` or `ss` on a dose row: "row {r} of subject {S} asks for additional doses or a steady state (`{column}`), which are not supported in this version; give each dose on its own row"), `several_endpoints` (DAT-CMT-01), `unsupported_censoring` (DAT-CEN-02) and `different_doses` (the existing sentence of the profile layout, DAT-LEG-01).
- Status: `assumed`
- Sources: design choice; S-39 (the values that are not supported are those the sources define: event types 2 to 4, negative rate and duration, `II`, `ADDL`, `SS`, summaries).

## 5. Covariates, missing values, BLQ and censoring

### DAT-COV-01 Covariates: declared, constant per subject
A covariate is a column that the user **declares** (role `covariate`, a list in the import dialog or the command); no column becomes a covariate by its name alone (the interface may suggest `wt`, `weight`, `age`, `sex`, `bsa`, `crcl`, `height` as candidates). A declared covariate is numeric or categorical (text); it has one value per subject (DAT-ERR-05); the value of a subject is its non-missing value. The covariates are shown in a subject table (one row per subject) next to the profiles, and carried unchanged into every per-subject result. Neither NCA nor the individual fit uses them now (DAT-USE-03); they are the input of the population brick.
- Status: `assumed`
- Sources: S-42 (`name.covariates` is a declared list, summary); S-37 (outer covariates, read in full); S-38 (`Theoph`: `Wt` and `Dose` are per-subject values repeated on every row); design choice.

### DAT-COV-02 Time-varying covariates: later
Covariates that change within a subject (a weight measured at each visit, a creatinine clearance) are **not accepted now**: the table is refused with `covariate_varies` if the column is declared, and an undeclared varying column is carried as `other`. The interpolation conventions of `rxode2` (carry the last value forward, the default; carry the next value backward; linear interpolation) are recorded for the card that adds them.
- Status: `later`
- Sources: S-39 (covariate interpolation methods, summary); S-37 (inner covariates).

### DAT-CEN-01 Missing values
A missing observation is an empty cell, or a marker the importer already treats as missing, or a row with `mdv` = 1. An observation row with a missing value (or `mdv` = 1) stays in the table with the reason `missing`, is counted as `n_missing` and is not used by any analysis (FIT-DAT-01). A dose row is not affected by `mdv`. A `mdv` value other than 0, 1 or empty is refused (`unknown_mdv`).
- Status: `documented, untested` for the meaning of the flag; `assumed` for the refusal of other values
- Sources: S-39 (`MDV`, summary); S-42 (`name.mdv`, summary); `specs/fit.md` FIT-DAT-01 (`n_missing`).

### DAT-CEN-02 BLQ and censoring columns
Two ways to mark a value below the limit of quantification are accepted, and they mean the same:
(a) **text in the value cell**, as today (`BLQ`, `<LOQ`, `< 0.5` …): the row is BLQ, the stored value is 0 with the BLQ flag (NCA-DAT-05c); a numeric limit written after `<` is kept as the limit of the row;
(b) **a `cens` column** with 1 (left-censored: the true value is below the limit): the value cell then holds the **limit** (the convention of `rxode2`, where DV carries the limit on a censored row), and the row is read as BLQ with that limit; the `limit` column, when present, is the other (lower) bound of the censoring interval, kept and unused now. `cens` = 0 or empty is an ordinary observation. `cens` = −1 (right-censored), or any other value, is refused (`unsupported_censoring`: "row {r} of subject {S} is marked right-censored (or has an unknown censoring code {v}); only left-censored values (1) are supported"). A censored row with `cens` = 1 and a missing limit is refused (`unsupported_censoring`, "the limit is missing").
In this version every BLQ row is **left out** of the analyses and counted (`n_blq`), by NCA according to its BLQ rules (`specs/nca.md` NCA-DAT-05) and by the fit as in FIT-DAT-01 and FIT-ML-19. The censored-likelihood method that would use the limit is brick 3 (`specs/fit.md` OF-21). The columns are specified now so that a table does not change when it arrives.
- Status: `documented, untested` for the `cens` and `limit` semantics; `assumed` for the mapping onto the BLQ flag
- Sources: S-41 (`CENS` and `LIMIT`, the old `nlmixr` censoring vignette and the `rxode2` page, summaries); S-42 (`name.cens`, summary); S-44 (BLQ methods, abstract summary); `specs/nca.md` NCA-DAT-05c.

## 6. Today's file in the new table

### DAT-LEG-01 The profile layout is the event table of a single dose at time 0
A profile-layout table maps into the event table as follows, per subject: one dose record at time 0 with amount = the value of the dose column (when the column exists and holds a value; a bolus, or an infusion when the user's model option says so), followed by every row as an observation record with its time, value, BLQ flag and missing flag. The dose column holds **one value per subject**: two different values in one subject stay the existing error ("subject {S} has two different doses ({a} and {b}) in column `{name}`; one dose per profile"), and a subject without a dose value has no dose record (a fit then asks for the dose as today). The mapping is a view for the analyses that need doses; **the worksheet, the file, the column roles, the notes and the stored form of a project are not changed by it.**
- Status: `assumed`
- Sources: the repo's worksheet (`profile()` and the one-dose rule); design choice.

### DAT-LEG-02 What must not change
Every file that loads today loads with the same columns, roles, notes and checks, in the same order, and gives the same result in every analysis: a dataset opened in the previous version gives the same NCA table, the same fit (estimates, statistics, trace), the same history entry and the same exported file after the brick as before it. This covers at least `oracle/data/theoph.csv`, `indometh.csv`, `edge_*.csv`, `synthetic_lz.csv`, the CSV and JSON of the fit oracle, and any project saved before the brick. A file is read as the event layout only if it contains a column recognised as `event`; the added keys of DAT-REC-01 (`cmt`, `rate`, `dur`, `mdv`, `cens`, `limit`) may be recognised in a profile-layout file that happens to contain such a column, with a note, and are not used by the analyses there. If recognising one of the added keys would change the result of a legacy file (a column called `limit` or `rate` in an existing file that today is `other`), the legacy behaviour wins: such a column stays `other` in the profile layout. The regression test is a byte-level comparison of worksheets, notes, checks, NCA outputs and fit outputs between the last version before the brick and the new one on all the files above (OD-03).
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
Roles (`subject`, `time`, `conc`, `amt`, `event`, `rate`, `dur`, `cmt`, `mdv`, `cens`, `limit`, `route`, `covariate`, `other`); event values (`obs`, `dose`; codes 0 and 1); check codes (`time_not_increasing`, `duplicate_times`, `dose_without_amount`, `amount_on_observation`, `subject_without_observation`, `covariate_varies`, `unknown_event`, `unsupported_event`, `non_positive_amount`, `rate_duration_mismatch`, `missing_time`, `empty_subject`, `unsupported_history`, `several_endpoints`, `unsupported_censoring`, `unknown_mdv`, `different_doses`); options (`covariates` = list of declared columns; `subject` = a label, none, or `all`). Role ids follow the style of `specs/nca.md` section 10 (lower case, underscores).
- Status: `assumed`
- Sources: the repo's role ids; `specs/nca.md` section 10 (naming style); design choice.

## 9. Later

### DAT-LAT-01 Described by the sources, outside the scope now
Listed with their sources; no rule, no test. (1) Event types 2, 3 and 4; additional doses and steady state (`II`, `ADDL`, `SS`); modelled rate and duration (negative codes): S-39. (2) Time-varying covariates and their interpolation: S-39, S-37. (3) Occasions (several occasions per subject): S-42 (`name.occ`). (4) Several observation endpoints and response types: S-42 (`name.ytype`), S-39 (`CMT`). (5) Right and interval censoring and the censored likelihood (M3, M4): S-41, S-44. (6) Unit conversion between the amount and the concentration units. (7) A sorting option applied at import.
- Status: `later`
- Sources: S-37, S-39, S-41, S-42, S-44 (summaries, as said in each).

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
| OD-07 | **Open design choices.** (1) The added name keys (`evt`, `tinf`, `lloq`, …) and whether `limit` and `rate` should ever be recognised in the profile layout (DAT-LEG-02). (2) Whether the table should accept `mdv` on dose rows. (3) Where the same-time order of a dose and an observation matters (the observation at the dose time is a pre-dose sample). (4) A unit-conversion layer (DAT-LAT-01). (5) Whether `subject = all` should be the default for NCA-like batches in the interface only. (6) Case-sensitivity of `rxode2` column names (not confirmed from a read page). | several | orchestrator |
