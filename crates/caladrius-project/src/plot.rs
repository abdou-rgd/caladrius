//! Plot objects (T-110): what an XY plot draws (its source and its few options) and the panels
//! it drew. A plot is an object of the workflow like an NCA or a fit: it is stored in the
//! project, named from its content, and marked stale when its source changes. Its points are the
//! source's values as they are; nothing is computed here or in the command that fills them.

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisId;
use crate::worksheet::WorksheetId;

/// Which plot of a fit result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitView {
    /// Observed concentrations and the fitted curve against time.
    #[default]
    Profile,
    /// Observed against predicted concentrations, with the identity line.
    ObservedPredicted,
    /// Residuals (observed minus predicted) against time, with the zero line.
    ResidualsTime,
    /// Weighted residuals against time, with the zero line.
    WeightedResidualsTime,
}

impl FitView {
    /// Every view, in the order a page offers them.
    pub const ALL: [FitView; 4] = [
        FitView::Profile,
        FitView::ObservedPredicted,
        FitView::ResidualsTime,
        FitView::WeightedResidualsTime,
    ];

    /// The id used in commands (`observed_predicted`).
    pub fn id(self) -> &'static str {
        match self {
            FitView::Profile => "profile",
            FitView::ObservedPredicted => "observed_predicted",
            FitView::ResidualsTime => "residuals_time",
            FitView::WeightedResidualsTime => "weighted_residuals_time",
        }
    }

    /// A short title.
    pub fn title(self) -> &'static str {
        match self {
            FitView::Profile => "Observed and fitted curve",
            FitView::ObservedPredicted => "Observed vs predicted",
            FitView::ResidualsTime => "Residuals vs time",
            FitView::WeightedResidualsTime => "Weighted residuals vs time",
        }
    }

    /// Residuals can be negative: they have no logarithmic axis.
    pub fn allows_log(self) -> bool {
        matches!(self, FitView::Profile | FitView::ObservedPredicted)
    }
}

/// Where the points of a plot come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlotSource {
    /// Concentration against time, from the cells of a worksheet.
    Worksheet {
        /// The worksheet read.
        worksheet: WorksheetId,
    },
    /// A diagnostic of a fit, from its stored result (never fitted again).
    Fit {
        /// The fit analysis read.
        analysis: AnalysisId,
        /// Which plot of the fit.
        #[serde(default)]
        view: FitView,
    },
}

/// The scale of the vertical axis (and of both axes for observed vs predicted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotScale {
    Linear,
    Log,
    /// A linear and a logarithmic panel side by side.
    #[default]
    Both,
}

impl PlotScale {
    /// The panels drawn for this scale: `false` linear, `true` logarithmic.
    pub fn panels(self) -> &'static [bool] {
        match self {
            PlotScale::Linear => &[false],
            PlotScale::Log => &[true],
            PlotScale::Both => &[false, true],
        }
    }
}

/// How the subjects of a worksheet plot are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotLayout {
    /// Every subject on the same axes.
    #[default]
    Overlay,
    /// One panel per subject.
    PerSubject,
}

/// An XY plot: its source and its options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotSpec {
    /// Where the points come from.
    pub source: PlotSource,
    /// The subjects drawn, in this order; `None` draws every subject of the worksheet. A fit has
    /// one subject, so this is ignored for a fit source.
    #[serde(default)]
    pub subjects: Option<Vec<String>>,
    /// Linear, logarithmic or both.
    #[serde(default)]
    pub scale: PlotScale,
    /// Overlaid or one panel per subject (worksheet source only).
    #[serde(default)]
    pub layout: PlotLayout,
    /// The title above the plot; `None` takes the label of the object.
    #[serde(default)]
    pub title: Option<String>,
}

/// An axis of a panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotAxis {
    /// What is on the axis (`Time`, `Concentration`, `Predicted`...).
    pub label: String,
    /// Its unit, from the worksheet, when it has one.
    #[serde(default)]
    pub unit: Option<String>,
    /// Logarithmic (base 10).
    #[serde(default)]
    pub log: bool,
}

impl PlotAxis {
    /// The axis title with its unit: `Time (h)`.
    pub fn title(&self) -> String {
        match self.unit.as_deref().map(str::trim) {
            Some(u) if !u.is_empty() => format!("{} ({u})", self.label),
            _ => self.label.clone(),
        }
    }
}

/// What a series is, which decides its colour and marker in the theme of whoever draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum SeriesRole {
    /// The subject at `slot` among the subjects of the worksheet (the slot keeps a subject's
    /// colour the same in every panel and every plot of that worksheet).
    Subject {
        /// Position of the subject in the worksheet, from 0.
        slot: usize,
    },
    /// Observed concentrations of a single profile.
    Observed,
    /// Predicted values of a fit.
    Predicted,
}

/// How a series is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesStyle {
    /// Markers only.
    Points,
    /// A line only.
    Line,
    /// Markers joined by a line.
    LinePoints,
}

/// One point, as it is in the source.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
    /// Below the limit of quantification (its `y` is 0, as stored in the worksheet).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub blq: bool,
}

/// One series of a panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotSeries {
    /// The name in the legend (a subject, `observed`, `predicted`).
    pub name: String,
    #[serde(flatten)]
    pub role: SeriesRole,
    pub style: SeriesStyle,
    /// In the order they are drawn (time order for a profile).
    pub points: Vec<PlotPoint>,
}

/// A reference line drawn under the series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotGuide {
    /// The line y = x (observed vs predicted).
    Identity,
    /// The line y = 0 (residuals).
    Zero,
}

/// One panel: axes and series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotPanel {
    /// Above the panel (`Linear`, `Semi-log`, `Subject 3, semi-log`).
    pub title: String,
    pub x: PlotAxis,
    pub y: PlotAxis,
    pub series: Vec<PlotSeries>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<PlotGuide>,
}

/// What a plot drew: its title and its panels, in reading order, and what it could not draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotRun {
    /// The title of the plot.
    pub title: String,
    /// The panels; for `both` the linear one comes before the logarithmic one.
    pub panels: Vec<PlotPanel>,
    /// What a person should know about the drawing (rows without a value, points a log axis
    /// cannot show), one sentence each.
    #[serde(default)]
    pub notes: Vec<String>,
}
