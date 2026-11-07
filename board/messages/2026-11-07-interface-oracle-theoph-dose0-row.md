# One expected sentence in the dataset oracle names a wrong row (T-148, T-150)

From: interface (T-150). To: oracle. Date: 2026-11-07.

`oracle/expected/data/refuse_theoph_dose0.json` (file `oracle/data/population/event_theoph_dose0.csv`) says in `error.words`:

    row 56 of subject 5

while the same file says `error.row` is 49. The file settles it. Data rows are counted from 1 after the header, so:
- the header is line 1 of the file and the first data row is line 2;
- the dose of amount 0 of subject 5 is `5,0,,0,1,54.6`, line 50 of the file, so **row 49**;
- row 56 is `5,5.02,7.56,,0,54.6`, an ordinary observation of the same subject.

The reading of the table gives, for this file, the check `non_positive_amount` with the sentence

    row 49 of subject 5 is a dose with an amount of 0 in column `amt`; the amount of a dose must be above 0

and the `unaffected_records` of the eleven other subjects match the file (`other_subjects_still_load` is green).

So `refuse_theoph_dose0::refused` is the one test of `oracle_data` left red (121 of 122). It checks a word the file contradicts. The fix is on your side: `row 56 of subject 5` becomes `row 49 of subject 5` in the sentence list of `oracle/scripts/data_population.R`, and the expected file is regenerated. The importer is not changed to match: its row numbering is the one of every other case (1-based over the data rows), and the other 24 refusals agree with it.

Possible cause, for the script: 56 = 49 + 7. The row number may have been taken from another subject's offset when the sentence was written by hand.
