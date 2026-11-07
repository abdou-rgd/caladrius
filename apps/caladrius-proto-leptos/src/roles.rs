//! The role chips of the data table. The roles come from the NCA scene of `plot.scene`: the series a
//! time falls in (`lambda_z_start`, `lambda_z_used`, `excluded`, `replaced`, `observed`), the points
//! below the limit it marks, and the points a logarithmic axis cannot show (listed under `hidden`).
//! Nothing is worked out here: a row with no place in the scene says so.

use std::collections::BTreeMap;

use caladrius_format as fmt;

use crate::types::{DataRow, NcaOk, Role, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Start,
    Used,
    Excluded,
    Replaced,
    Observed,
    Blq,
    Tmax,
    Removed,
    None,
}

impl ChipKind {
    pub fn css(self) -> &'static str {
        match self {
            ChipKind::Start => "start",
            ChipKind::Used => "used",
            ChipKind::Excluded => "excluded",
            ChipKind::Replaced => "replaced",
            ChipKind::Observed => "observed",
            ChipKind::Blq => "blq",
            ChipKind::Tmax => "tmax",
            ChipKind::Removed => "removed",
            ChipKind::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub kind: ChipKind,
    pub text: String,
}

/// The roles of a table row, strongest first.
const RANK: [Role; 5] = [
    Role::LambdaZStart,
    Role::LambdaZUsed,
    Role::Excluded,
    Role::Replaced,
    Role::Observed,
];

fn rank(role: Role) -> Option<usize> {
    RANK.iter().position(|r| *r == role)
}

fn kind_of(role: Role) -> ChipKind {
    match role {
        Role::LambdaZStart => ChipKind::Start,
        Role::LambdaZUsed => ChipKind::Used,
        Role::Excluded => ChipKind::Excluded,
        Role::Replaced => ChipKind::Replaced,
        _ => ChipKind::Observed,
    }
}

/// The key of a time in a map: the bits of the double (a time is compared as the engine wrote it).
fn key(time: f64) -> u64 {
    time.to_bits()
}

/// The role of each time of the subject, from the first panel of the scene.
pub fn roles_by_time(scene: Option<&Scene>) -> BTreeMap<u64, (Role, bool)> {
    let mut out: BTreeMap<u64, (Role, bool)> = BTreeMap::new();
    let Some(scene) = scene else { return out };
    let Some(panel) = scene.panels.first() else {
        return out;
    };
    let mut consider = |time: f64, role: Role, blq: bool| match out.get_mut(&key(time)) {
        None => {
            out.insert(key(time), (role, blq));
        }
        Some(have) => {
            let stronger = rank(role)
                .zip(rank(have.0))
                .is_some_and(|(new, old)| new < old);
            if stronger {
                *have = (role, blq || have.1);
            } else if blq {
                have.1 = true;
            }
        }
    };
    for s in &panel.series {
        if rank(s.role).is_none() {
            continue;
        }
        for p in &s.points {
            consider(p.x, s.role, p.blq);
        }
    }
    let name_to_role: BTreeMap<&str, Role> = scene
        .legend
        .entries
        .iter()
        .map(|e| (e.name.as_str(), e.role))
        .collect();
    if let Some(hidden) = &panel.hidden {
        for h in &hidden.points {
            if let Some(role) = name_to_role.get(h.series.as_str())
                && rank(*role).is_some()
            {
                consider(h.x, *role, false);
            }
        }
    }
    out
}

/// The legend name of a role, as the engine wrote it.
fn name_of(scene: &Scene, role: Role) -> String {
    scene
        .legend
        .entries
        .iter()
        .find(|e| e.role == role)
        .map_or_else(|| role.css().to_owned(), |e| e.name.clone())
}

/// The chips of every row of the subject (keyed by the row number of the worksheet).
pub fn chips_for(
    rows: &[DataRow],
    scene: Option<&Scene>,
    ok: Option<&NcaOk>,
    tmax: Option<f64>,
) -> BTreeMap<usize, Vec<Chip>> {
    let roles = roles_by_time(scene);
    let removed: BTreeMap<u64, &str> = ok
        .map(|o| {
            o.removed
                .iter()
                .map(|r| (key(r.time), r.reason.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let mut out = BTreeMap::new();
    for row in rows {
        let mut chips = Vec::new();
        if let (Some(scene), Some((role, blq))) = (scene, roles.get(&key(row.time))) {
            chips.push(Chip {
                kind: kind_of(*role),
                text: name_of(scene, *role),
            });
            if *blq {
                chips.push(Chip {
                    kind: ChipKind::Blq,
                    text: "BLQ".to_owned(),
                });
            }
        } else if let Some(reason) = removed.get(&key(row.time)) {
            chips.push(Chip {
                kind: ChipKind::Removed,
                text: fmt::words(reason),
            });
        }
        if tmax == Some(row.time) {
            chips.push(Chip {
                kind: ChipKind::Tmax,
                text: "Tmax".to_owned(),
            });
        }
        if chips.is_empty() {
            chips.push(Chip {
                kind: ChipKind::None,
                text: "-".to_owned(),
            });
        }
        out.insert(row.index, chips);
    }
    out
}
