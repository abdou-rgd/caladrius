//! Names for the eye: labels and units of parameters, sentences of the engine, error codes.

use std::collections::BTreeMap;

/// The label of a PKNCA parameter name in the summary: `lambda.z` is `λz`, `aucinf.obs` is
/// `AUCinf`. Names the table does not know are shown as they are.
pub fn parameter_label(name: &str, extravascular: bool) -> String {
    let f = if extravascular { "/F" } else { "" };
    match name {
        "cmax" => "Cmax".to_owned(),
        "tmax" => "Tmax".to_owned(),
        "tlast" => "Tlast".to_owned(),
        "clast.obs" => "Clast".to_owned(),
        "auclast" => "AUClast".to_owned(),
        "aucall" => "AUCall".to_owned(),
        "aucinf.obs" => "AUCinf".to_owned(),
        "aucinf.pred" => "AUCinf (predicted Clast)".to_owned(),
        "aucpext.obs" => "AUC extrapolated".to_owned(),
        "aucpext.pred" => "AUC extrapolated (predicted Clast)".to_owned(),
        "lambda.z" => "λz".to_owned(),
        "half.life" => "t½".to_owned(),
        "r.squared" => "R²".to_owned(),
        "adj.r.squared" => "Adjusted R²".to_owned(),
        "span.ratio" => "Span ratio".to_owned(),
        "lambda.z.n.points" => "Points in λz".to_owned(),
        "mrt.obs" => "MRT".to_owned(),
        "mrt.iv.obs" => "MRT (IV)".to_owned(),
        "cl.obs" => format!("CL{f}"),
        "vz.obs" => format!("Vz{f}"),
        "vss.iv.obs" => "Vss".to_owned(),
        "c0" => "C0".to_owned(),
        other => other.to_owned(),
    }
}

/// A sentence of the engine with the parameter names it quotes written as the results table
/// labels them (`aucpext.obs` is `AUC extrapolated`).
pub fn with_labels(sentence: &str) -> String {
    // Longer names first: `lambda.z.n.points` before `lambda.z`.
    const NAMES: [&str; 9] = [
        "lambda.z.n.points",
        "aucpext.pred",
        "aucpext.obs",
        "adj.r.squared",
        "span.ratio",
        "aucinf.pred",
        "aucinf.obs",
        "lambda.z",
        "auclast",
    ];
    let mut out = sentence.to_owned();
    for name in NAMES {
        out = out.replace(name, &parameter_label(name, true));
    }
    out
}

/// The unit of a parameter from the units of the worksheet columns and the derived units the
/// engine names; `None` when it has none or the units are not known.
pub fn parameter_unit(
    name: &str,
    time: Option<&str>,
    conc: Option<&str>,
    derived: &BTreeMap<String, String>,
) -> Option<String> {
    let derived = |key: &str| derived.get(key).cloned();
    match name {
        "cmax" | "clast.obs" | "clast.pred" | "c0" => conc.map(str::to_owned),
        "tmax"
        | "tlast"
        | "tfirst"
        | "tlag"
        | "half.life"
        | "mrt.obs"
        | "mrt.pred"
        | "mrt.last"
        | "mrt.iv.obs"
        | "mrt.iv.pred"
        | "mrt.iv.last"
        | "lambda.z.time.first"
        | "lambda.z.time.last" => time.map(str::to_owned),
        "auclast" | "aucall" | "aucinf.obs" | "aucinf.pred" => derived("auc"),
        "aumclast" | "aumcall" | "aumcinf.obs" | "aumcinf.pred" => derived("aumc"),
        "lambda.z" => derived("lambda_z"),
        "cl.obs" | "cl.pred" => derived("cl"),
        "vz.obs" | "vz.pred" | "vss.iv.obs" | "vss.iv.pred" | "vss.obs" | "vss.pred"
        | "vss.iv.last" => derived("v"),
        "aucpext.obs" | "aucpext.pred" | "aumcpext.obs" | "aumcpext.pred" => Some("%".to_owned()),
        _ => None,
    }
}

/// A sentence without the backticks the engine puts around identifiers: `v` is shown as v.
pub fn plain(text: &str) -> String {
    text.replace('`', "")
}

/// `snake_case` as words: `no_valid_fit` is `no valid fit`.
pub fn words(code: &str) -> String {
    code.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_sentences_use_the_labels_of_the_results_table() {
        assert_eq!(
            with_labels(
                "aucpext.obs is 37.3 % (> 20 %); the AUC to infinity relies mostly on extrapolation"
            ),
            "AUC extrapolated is 37.3 % (> 20 %); the AUC to infinity relies mostly on extrapolation"
        );
        assert!(
            with_labels("aucpext.pred is 5 %").starts_with("AUC extrapolated (predicted Clast) is")
        );
        assert_eq!(with_labels("nothing to change"), "nothing to change");
    }

    #[test]
    fn labels_and_units_follow_the_route_and_the_columns() {
        assert_eq!(parameter_label("cl.obs", true), "CL/F");
        assert_eq!(parameter_label("cl.obs", false), "CL");
        assert_eq!(parameter_label("lambda.z", true), "λz");
        assert_eq!(parameter_label("something.else", true), "something.else");
        let derived: BTreeMap<String, String> = [
            ("auc".to_owned(), "h*mg/L".to_owned()),
            ("cl".to_owned(), "L/h".to_owned()),
        ]
        .into();
        assert_eq!(
            parameter_unit("auclast", Some("h"), Some("mg/L"), &derived).as_deref(),
            Some("h*mg/L")
        );
        assert_eq!(
            parameter_unit("cmax", Some("h"), Some("mg/L"), &derived).as_deref(),
            Some("mg/L")
        );
        assert_eq!(
            parameter_unit("half.life", Some("h"), None, &derived).as_deref(),
            Some("h")
        );
        assert_eq!(
            parameter_unit("aucpext.obs", None, None, &derived).as_deref(),
            Some("%")
        );
        assert_eq!(parameter_unit("vz.obs", Some("h"), None, &derived), None);
        assert_eq!(parameter_unit("r.squared", None, None, &derived), None);
        assert_eq!(words("no_valid_fit"), "no valid fit");
        assert_eq!(plain("`v` is 3"), "v is 3");
    }
}
