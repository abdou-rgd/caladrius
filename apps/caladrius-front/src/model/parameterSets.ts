// What the model panel knows about the models: the routes, the parameter sets of each number of
// compartments, and a label and a unit for each parameter name.
//
// This is UI knowledge kept in one data file on purpose. The model ids themselves come from the engine
// (the `ModelId` enum of `fit.run`'s `params_schema`, see registry.ts) and a model the engine does not list is
// not offered. A bolder step: an engine command that describes the models (parameter sets, labels,
// units, the diagram) should replace this file. The `a, b, alpha, beta` macro-constant sets are left out tonight.

export type Compartments = 1 | 2 | 3;

export type Route = 'iv_bolus' | 'iv_infusion' | 'oral_1' | 'oral_1_lag' | 'oral_0' | 'oral_0_lag';

export interface RouteInfo {
  id: Route;
  label: string;
  /** Volumes and clearances are apparent (divided by F). */
  extravascular: boolean;
  /** Parameters this route adds to the compartment set, in the order shown. */
  extra: string[];
}

export const ROUTES: RouteInfo[] = [
  { id: 'iv_bolus', label: 'IV bolus', extravascular: false, extra: [] },
  { id: 'iv_infusion', label: 'IV infusion', extravascular: false, extra: ['dur'] },
  { id: 'oral_1', label: 'Oral, first order', extravascular: true, extra: ['ka'] },
  { id: 'oral_1_lag', label: 'Oral, first order, lag time', extravascular: true, extra: ['ka', 'tlag'] },
  { id: 'oral_0', label: 'Oral, zero order', extravascular: true, extra: ['dur'] },
  { id: 'oral_0_lag', label: 'Oral, zero order, lag time', extravascular: true, extra: ['dur', 'tlag'] },
];

export interface ParameterSet {
  id: string;
  label: string;
  names: string[];
}

/** The parameterisations of the disposition, by number of compartments. The first of each is the default. */
export const SETS: Record<Compartments, ParameterSet[]> = {
  1: [
    { id: 'cl', label: 'Clearance', names: ['v', 'cl'] },
    { id: 'k', label: 'Rate constant', names: ['v', 'k'] },
  ],
  2: [
    { id: 'cl', label: 'Clearances', names: ['cl', 'vc', 'q', 'vp'] },
    { id: 'k', label: 'Rate constants', names: ['k10', 'k12', 'k21', 'vc'] },
  ],
  3: [
    { id: 'cl', label: 'Clearances', names: ['cl', 'vc', 'q2', 'vp2', 'q3', 'vp3'] },
    { id: 'k', label: 'Rate constants', names: ['k10', 'k12', 'k21', 'k13', 'k31', 'vc'] },
  ],
};

export const COMPARTMENTS: Compartments[] = [1, 2, 3];

export function routeInfo(route: Route): RouteInfo {
  return ROUTES.find((r) => r.id === route) ?? ROUTES[0]!;
}

/** The engine's model id: `pk3.iv_bolus`. */
export function modelId(compartments: Compartments, route: Route): string {
  return `pk${compartments}.${route}`;
}

/** The set of a number of compartments by id; the first one when the id is not known for it. */
export function setOf(compartments: Compartments, setId: string): ParameterSet {
  const sets = SETS[compartments];
  return sets.find((s) => s.id === setId) ?? sets[0]!;
}

/** The parameter names the person gives values for: the disposition set, then what the route adds. */
export function parameterNames(compartments: Compartments, route: Route, setId: string): string[] {
  return [...setOf(compartments, setId).names, ...routeInfo(route).extra];
}

/** Every (compartments, route) whose model id the engine lists. */
export function offeredModels(engineModels: readonly string[]): { compartments: Compartments; route: Route }[] {
  const out: { compartments: Compartments; route: Route }[] = [];
  for (const compartments of COMPARTMENTS) {
    for (const r of ROUTES) if (engineModels.includes(modelId(compartments, r.id))) out.push({ compartments, route: r.id });
  }
  return out;
}

const LABELS: Record<string, string> = {
  v: 'V',
  vc: 'Vc',
  vp: 'Vp',
  vp2: 'Vp2',
  vp3: 'Vp3',
  cl: 'CL',
  q: 'Q',
  q2: 'Q2',
  q3: 'Q3',
  k: 'k',
  k10: 'k10',
  k12: 'k12',
  k21: 'k21',
  k13: 'k13',
  k31: 'k31',
  ka: 'ka',
  tlag: 'tlag',
  dur: 'T (duration)',
};

const APPARENT = new Set(['v', 'vc', 'vp', 'vp2', 'vp3', 'cl', 'q', 'q2', 'q3']);

/** The label of a parameter; volumes and clearances are shown divided by F on an extravascular route. */
export function parameterLabel(name: string, extravascular: boolean): string {
  const base = LABELS[name] ?? name;
  return extravascular && APPARENT.has(name) ? `${base}/F` : base;
}

export interface UnitContext {
  time?: string | undefined;
  conc?: string | undefined;
  /** The worksheet's `derived_units`: `v`, `cl`, `auc`... */
  derived: Record<string, string>;
}

/** The unit of a parameter from the worksheet's units; undefined when it has none or they are not known. */
export function parameterUnit(name: string, units: UnitContext): string | undefined {
  switch (name) {
    case 'v':
    case 'vc':
    case 'vp':
    case 'vp2':
    case 'vp3':
      return units.derived['v'];
    case 'cl':
    case 'q':
    case 'q2':
    case 'q3':
      return units.derived['cl'];
    case 'k':
    case 'ka':
    case 'k10':
    case 'k12':
    case 'k21':
    case 'k13':
    case 'k31':
      return units.time === undefined ? undefined : `1/${units.time}`;
    case 'tlag':
    case 'dur':
      return units.time;
    default:
      return undefined;
  }
}
