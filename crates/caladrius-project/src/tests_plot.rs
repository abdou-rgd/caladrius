//! Tests of plot objects in the project (T-110): validation of the source, labels, stale marking
//! when the worksheet or the fit read changes, removal rules, save and load.

use crate::*;

const CSV: &str = "Subject,Time (h),Conc (ng/mL),Dose (mg)\n\
A,0,0,50\nA,1,6,50\nA,3,4,50\nB,0,0,50\nB,1,5,50\nB,3,2,50\n";

fn project() -> (Project, WorksheetId) {
    let table = ImportedTable::from_csv(CSV.as_bytes(), &CsvOptions::default()).unwrap();
    let mut project = Project::new("plots");
    let ws = project.add_worksheet("invented", table.columns).unwrap();
    (project, ws)
}

fn sheet_plot(worksheet: WorksheetId, subjects: Option<Vec<&str>>) -> AnalysisSpec {
    AnalysisSpec::Plot(PlotSpec {
        source: PlotSource::Worksheet { worksheet },
        subjects: subjects.map(|v| v.into_iter().map(str::to_owned).collect()),
        scale: PlotScale::Both,
        layout: PlotLayout::Overlay,
        title: None,
    })
}

fn fit_plot(analysis: AnalysisId, view: FitView) -> AnalysisSpec {
    AnalysisSpec::Plot(PlotSpec {
        source: PlotSource::Fit { analysis, view },
        subjects: None,
        scale: PlotScale::Linear,
        layout: PlotLayout::Overlay,
        title: None,
    })
}

fn fit_spec(worksheet: WorksheetId) -> AnalysisSpec {
    AnalysisSpec::Fit(FitSpec {
        worksheet,
        subject: Some("A".to_owned()),
        model: caladrius_models::ModelId::Oral1,
        dose: None,
        weighting: Default::default(),
        initial: Default::default(),
        options: Default::default(),
    })
}

/// A plot result with nothing in it: the project stores whatever the engine drew.
fn drawn() -> AnalysisResult {
    AnalysisResult::Plot(PlotRun {
        title: "t".to_owned(),
        panels: Vec::new(),
        notes: Vec::new(),
    })
}

/// A fit result is not needed to test the bookkeeping: any result marks a run.
fn some_result() -> AnalysisResult {
    AnalysisResult::Simulation(SimulationRun {
        times: Vec::new(),
        outcome: Outcome::Error("not needed here".to_owned()),
    })
}

#[test]
fn a_plot_is_an_analysis_named_from_its_source() {
    let (mut p, ws) = project();
    let all = p.add_analysis(sheet_plot(ws, None), None).unwrap();
    let one = p
        .add_analysis(sheet_plot(ws, Some(vec!["B"])), None)
        .unwrap();
    let two = p
        .add_analysis(sheet_plot(ws, Some(vec!["A", "B"])), None)
        .unwrap();
    assert_eq!(p.analysis(all).unwrap().spec().kind(), "plot");
    assert_eq!(p.label_of(all).unwrap(), "Plot of invented, all subjects");
    assert_eq!(p.label_of(one).unwrap(), "Plot of invented, subject B");
    assert_eq!(p.label_of(two).unwrap(), "Plot of invented, 2 subjects");
    let fit = p.add_analysis(fit_spec(ws), None).unwrap();
    let res = p
        .add_analysis(fit_plot(fit, FitView::ResidualsTime), None)
        .unwrap();
    assert_eq!(
        p.label_of(res).unwrap(),
        "Residuals vs time of Fit pk1.oral_1 to invented, subject A"
    );
}

#[test]
fn a_plot_reads_an_existing_worksheet_or_fit_and_nothing_else() {
    let (mut p, ws) = project();
    assert_eq!(
        p.add_analysis(sheet_plot(WorksheetId(42), None), None)
            .unwrap_err()
            .code,
        "unknown_worksheet"
    );
    assert_eq!(
        p.add_analysis(fit_plot(AnalysisId(42), FitView::Profile), None)
            .unwrap_err()
            .code,
        "unknown_analysis"
    );
    // A plot of a plot, or of an NCA, is refused with the way out.
    let plot = p.add_analysis(sheet_plot(ws, None), None).unwrap();
    let err = p
        .add_analysis(fit_plot(plot, FitView::Profile), None)
        .unwrap_err();
    assert_eq!(err.code, "wrong_plot_source");
    assert!(err.message.contains("a plot reads a worksheet or a fit"));
    // Changing a plot to read a missing fit is refused too, and changes nothing.
    assert_eq!(
        p.update_spec(plot, fit_plot(AnalysisId(42), FitView::Profile))
            .unwrap_err()
            .code,
        "unknown_analysis"
    );
    assert_eq!(p.analysis(plot).unwrap().spec(), &sheet_plot(ws, None));
}

#[test]
fn a_worksheet_plot_is_stale_when_the_data_change() {
    let (mut p, ws) = project();
    let plot = p.add_analysis(sheet_plot(ws, None), None).unwrap();
    assert_eq!(p.status(plot).unwrap(), AnalysisStatus::NoResult);
    p.set_result(plot, drawn()).unwrap();
    assert_eq!(p.status(plot).unwrap(), AnalysisStatus::Fresh);
    p.edit_worksheet(ws, |w| w.set_number("Conc", 1, Some(7.0)))
        .unwrap();
    assert!(matches!(
        p.status(plot).unwrap(),
        AnalysisStatus::Stale { reasons } if matches!(reasons.as_slice(), [StaleReason::InputChanged { .. }])
    ));
    // A worksheet read by a plot cannot be removed before the plot.
    assert_eq!(p.remove_worksheet(ws).unwrap_err().code, "worksheet_in_use");
}

#[test]
fn a_fit_plot_is_stale_when_the_fit_runs_again_or_goes_stale() {
    let (mut p, ws) = project();
    let fit = p.add_analysis(fit_spec(ws), None).unwrap();
    p.set_result(fit, some_result()).unwrap();
    let plot = p
        .add_analysis(fit_plot(fit, FitView::ObservedPredicted), None)
        .unwrap();
    p.set_result(plot, drawn()).unwrap();
    assert_eq!(p.status(plot).unwrap(), AnalysisStatus::Fresh);

    // The fit runs again: the plot shows an older result.
    p.set_result(fit, some_result()).unwrap();
    assert_eq!(
        p.status(plot).unwrap(),
        AnalysisStatus::Stale {
            reasons: vec![StaleReason::SourceChanged]
        }
    );
    p.set_result(plot, drawn()).unwrap();
    assert_eq!(p.status(plot).unwrap(), AnalysisStatus::Fresh);

    // The data of the fit change: the fit is stale, so the plot says so too.
    p.edit_worksheet(ws, |w| w.set_number("Conc", 2, Some(3.5)))
        .unwrap();
    assert_eq!(
        p.status(plot).unwrap(),
        AnalysisStatus::Stale {
            reasons: vec![StaleReason::SourceStale]
        }
    );
    assert!(StaleReason::SourceStale.message().contains("run it again"));

    // A fit shown by a plot is kept until the plot goes.
    let err = p.remove_analysis(fit).unwrap_err();
    assert_eq!(err.code, "analysis_in_use");
    assert!(err.message.contains("Observed vs predicted"));
    p.remove_analysis(plot).unwrap();
    p.remove_analysis(fit).unwrap();
}

#[test]
fn plots_round_trip_through_a_project_file() {
    let (mut p, ws) = project();
    let fit = p.add_analysis(fit_spec(ws), None).unwrap();
    p.set_result(fit, some_result()).unwrap();
    let a = p
        .add_analysis(sheet_plot(ws, Some(vec!["A"])), Some("mine"))
        .unwrap();
    let b = p
        .add_analysis(fit_plot(fit, FitView::WeightedResidualsTime), None)
        .unwrap();
    p.set_result(
        a,
        AnalysisResult::Plot(PlotRun {
            title: "mine".to_owned(),
            panels: vec![PlotPanel {
                title: "Linear".to_owned(),
                x: PlotAxis {
                    label: "Time".to_owned(),
                    unit: Some("h".to_owned()),
                    log: false,
                },
                y: PlotAxis {
                    label: "Concentration".to_owned(),
                    unit: None,
                    log: false,
                },
                series: vec![PlotSeries {
                    name: "A".to_owned(),
                    role: SeriesRole::Subject { slot: 0 },
                    style: SeriesStyle::LinePoints,
                    points: vec![
                        PlotPoint {
                            x: 0.0,
                            y: 0.0,
                            blq: true,
                        },
                        PlotPoint {
                            x: 1.0,
                            y: 6.0,
                            blq: false,
                        },
                    ],
                }],
                guide: Some(PlotGuide::Zero),
            }],
            notes: vec!["a note".to_owned()],
        }),
    )
    .unwrap();
    p.set_result(b, drawn()).unwrap();
    p.set_result(fit, some_result()).unwrap();
    let bytes = p.to_bytes().unwrap();
    let back = Project::from_bytes(&bytes).unwrap();
    assert_eq!(back, p);
    assert_eq!(back.status(a).unwrap(), AnalysisStatus::Fresh);
    assert_eq!(
        back.status(b).unwrap(),
        AnalysisStatus::Stale {
            reasons: vec![StaleReason::SourceChanged]
        }
    );
    // A BLQ flag is written only where it is true.
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("\"blq\": true").count(), 1);
    assert!(!text.contains("\"blq\": false"));

    // A file whose plot reads an NCA (or nothing) is refused.
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let analyses = doc["analyses"].as_array_mut().unwrap();
    let plot = analyses
        .iter_mut()
        .find(|x| x["spec"]["source"]["kind"] == "fit")
        .unwrap();
    plot["spec"]["source"]["analysis"] = serde_json::json!(a.0);
    let broken = serde_json::to_vec(&doc).unwrap();
    assert_eq!(
        Project::from_bytes(&broken).unwrap_err().code,
        "wrong_plot_source"
    );
}

#[test]
fn a_file_written_before_plots_still_loads() {
    // `runs` and `source_run` are absent from older files.
    let (mut p, ws) = project();
    let fit = p.add_analysis(fit_spec(ws), None).unwrap();
    p.set_result(fit, some_result()).unwrap();
    let mut doc: serde_json::Value = serde_json::from_slice(&p.to_bytes().unwrap()).unwrap();
    let a = &mut doc["analyses"][0];
    a.as_object_mut().unwrap().remove("runs");
    assert!(a["stored"].get("source_run").is_none());
    let back = Project::from_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(back.status(fit).unwrap(), AnalysisStatus::Fresh);
}

#[test]
fn options_have_defaults_and_ids() {
    let spec: PlotSpec = serde_json::from_value(serde_json::json!({
        "source": { "kind": "worksheet", "worksheet": 1 }
    }))
    .unwrap();
    assert_eq!(spec.scale, PlotScale::Both);
    assert_eq!(spec.layout, PlotLayout::Overlay);
    assert_eq!(spec.subjects, None);
    assert_eq!(PlotScale::Both.panels(), &[false, true]);
    let ids: Vec<&str> = FitView::ALL.iter().map(|v| v.id()).collect();
    for (view, id) in FitView::ALL.iter().zip(&ids) {
        assert_eq!(serde_json::to_value(view).unwrap(), serde_json::json!(id));
    }
    assert!(FitView::Profile.allows_log() && !FitView::ResidualsTime.allows_log());
    let bad = serde_json::from_value::<PlotSpec>(serde_json::json!({
        "source": { "kind": "worksheet", "worksheet": 1 }, "colour": "red"
    }));
    assert!(bad.is_err());
    assert_eq!(
        PlotAxis {
            label: "Time".to_owned(),
            unit: Some(" ".to_owned()),
            log: false
        }
        .title(),
        "Time"
    );
}
