# Long-running commands over the bridge

Status: spec, nothing implemented (T-141; the engine side is T-143). Today the bridge (`apps/caladrius-bridge`, T-134) has one call, `execute(request) -> sealed answer`, and the Tauri shell wraps it in one `#[tauri::command(async)]`. A population fit lasts minutes, so it needs a job id, progress, and cancel. Rules 6 (an error is a sentence) and 10 (same bytes, same seal everywhere) of `AGENTS.md` bind every line below.

## 1. The shape

| verb | in | out |
|---|---|---|
| `fit.start` | the parameters of `fit.run`, unchanged | at once: `{"job":"j1"}`, or the standard refusal (`ok:false`, no job) when the parameters are invalid |
| `fit.cancel` | `{"job":"j1"}` | at once: `{"requested":true}`; `false` if the job is unknown or already ended |

`fit.start` and `fit.cancel` are bridge verbs, not engine commands: they never enter the history. Any later long command `X` follows the same rule (`X.start`, `X.cancel`). The host chooses the job id (a counter: no clock, no random number); it is never sealed.

The job then emits ordered events, each tagged with `job`:
- `progress`: `{kind, job, iteration, objective_name ("wrss" individual, "ofv" population), objective, params}`. One per iteration, `iteration` strictly increasing. Not sealed, not in the history, free to drop on the host side.
- exactly one terminal event, then silence: `done` carries the standard bridge answer of `fit.run` (`ok` true or false, with its seal); or `cancelled`: `{kind, job, after_iteration:n, message:"fit cancelled after iteration n"}` plus `last`, a copy of the last `progress`, and `trace`, the rows of the minimization trace (`specs/fit.md` FIT-OUT-11) from iteration 0 to n. Neither is an answer: no seal, not in the history.

A `done` answer is the answer `fit.run` would have given with the same parameters and project, byte for byte, and the history gets one `fit.run` entry when the job ends. A cancelled job records nothing. A `fit.start` refused for invalid parameters is recorded as a failed `fit.run` entry, exactly as `execute` records it today, so the history is the same as with `execute`.

**The project cannot move under a job.** `fit.run` is `mutates: true` (it stores an analysis; its answer holds the analysis id and freshness), and `Engine::execute` hashes the inputs and records the entry in one step. An entry recorded at the end of a job would replay against another project (`diverged`) if a mutating command had run meanwhile. So, while a job runs, every command that mutates the project is refused with a sentence naming the running job; read-only commands (views, plots, exports) still run. As a second check, `begin` records `engine.revision()` and `finish` refuses, with a sentence, if it changed. The project the fit saw at `begin` is thus the project at `finish`, and the entry replays to the same seal.

## 2. The engine side (for T-143, `caladrius-fit` and `caladrius-engine`)

One primitive, a stepper that owns its state as a value, and thin wrappers over it, so that there is one loop:

```rust
pub struct FitRun<M: FitModel> { /* the model by value, the data, and the loop state below */ }
impl<M: FitModel> FitRun<M> { pub fn new(model: M, input: &FitInput) -> Result<Self, FitError>;  // as Problem::new
                              pub fn step(&mut self) -> Step<'_>; }   // exactly one iteration of the loop of `solve`
pub enum Step<'a> { Progress(Progress<'a>), Finished(FitResult) }
pub struct Progress<'a> { pub iteration: u32, pub objective: f64, pub names: &'a [String], pub values: &'a [f64] }
pub trait Observer { fn after_iteration(&mut self, p: &Progress) -> Control; }  // Control::{Continue, Cancel}
pub enum Outcome { Done(FitResult), Cancelled { after_iteration: u32, trace: Vec<TraceRow> } }
impl<M: FitModel> FitRun<M> { pub fn run(self, obs: &mut dyn Observer) -> Outcome; }
// blanket `impl<M: FitModel + ?Sized> FitModel for &M`: `run_model(&dyn FitModel, &FitInput)` keeps its
// signature (its callers `Plain` and `WithClosedForms` keep working); `solve` and `run_model` call `run`
// with an observer that never cancels. A host that needs `'static` passes a `ModelId` by value.
```

T-143 may choose another way, provided `run_model` keeps its signature and there is one loop. `Step` borrows from the run, so the host serialises the event before the next `step`. `FitRun` carries, at least, everything `solve` carries across iterations (`engine.rs`): `theta`, `pred`, the weights, `lambda`, the trace, `previous_decrease` (the FIT-CNV-05 test), `status`, `ss_weighted_start`, `gain_floor`, and anything else the loop holds. Forgetting one changes the iterates, so it breaks rule 10. The block after the loop (rollback to the best iterate, then `result::build`) belongs to the step that returns `Finished`.

The cancel flag is read in `run` (native) or by the host (Worker, section 3) only between two `step` calls, after the iteration has committed `theta` and `trace` and before the next one begins; never inside the line search, the Jacobian or an ODE solve. The population fit (FOCE) gets the same shape, but one outer iteration of FOCE may take far longer than the cancel threshold of wave 4; the cancel granularity for FOCE is open (a check inside the outer iteration, between the inner problems of the subjects, is a candidate for the population spec T-151). At the engine level: `Engine::begin(command, params) -> Result<Job, CommandError>` validates, takes a snapshot of what the command reads and `engine.revision()` (the job is `Send` and holds no lock), and `Engine::finish(job, outcome) -> Result<Value, CommandError>` checks the revision, records the entry and seals it. The seal is computed from the command id, the parameters and the answer, given the project the job saw, so it is independent of when, where and how the job ran.

Why an observer that never cancels changes nothing (rule 10): `Progress` borrows values the iteration has already computed (names and values separately, nothing allocated per iteration) and the observer returns only `Control`; the engine reads no clock, thread or flag of its own, so the sequence of floating-point operations is the same with or without it; `Progress` and `Cancelled` never enter the answer. T-143 proves it with tests: canonical bytes and seal of a run with a recording observer equal those of `solve()`; a loop of `step` equals `solve()`; cancel at iteration n returns `Cancelled{n}` and leaves the history unchanged; no `unwrap`, no `panic` on any path (rule 6).

## 3. The Web Worker case: one position

A synchronous engine call blocks the worker's event loop, so a `cancel` message is not read until it returns (card T-141, point 3; Fable message, placement B). **Position: the host drives the job one iteration at a time; `Worker.terminate()` is not the cancel path.** The worker holds the `FitRun` in the bridge's wasm memory under the job id, runs one `step`, posts the `progress` event, and yields to the event loop with a macrotask (a `MessageChannel` self-post) before the next step. A `cancel` message is a task like any other: it is handled between two steps, sets the flag, and the next tick emits `cancelled` instead of stepping. Latency of a cancel is therefore at most one iteration (to be measured in wave 4, S3, against the 200 ms threshold of the Fable message; no figure is claimed here).

Reasons: `terminate()` stops the worker at once and gives it no opportunity to finish its operations (MDN, paraphrased), so the state, the trace, `last` and the project in the module's memory die with it, and the page would re-instantiate the module and re-load the project to carry on. SharedArrayBuffer is off in WebKitGTK (Fable message, placement C), so a shared flag is not available. Cost to T-143: `FitRun` must own everything, model included (a `'static` model such as a `ModelId`, where today `Problem::new` borrows `&dyn FitModel`), and `step` must return after one iteration; the same API serves the native observer, so nothing is Worker-specific. `terminate()` stays only as a watchdog for a single iteration that never returns; it ends the job with `cancelled` built from the host's last `progress`, and costs the module.

## 4. Three transports (described, none implemented)

**Tauri 2** (desktop, placement D). `fit_start(state: State<'_, EngineState>, jobs: State<'_, Jobs>, request: String, on_event: Channel<JobEvent>) -> Result<String, String>` is a `#[tauri::command(async)]` (an async command with borrowed arguments such as `State<'_, T>` must return a `Result`) that calls `Engine::begin` under the shell's `Mutex<Bridge>`, registers `Arc<AtomicBool>` under the job id in managed state (`.manage`), spawns the blocking work on the async runtime, and returns the job id at once. The task runs `FitRun::run` with an observer that sends each `Progress` through `on_event.send(..)` and returns `Cancel` when the flag is set, then takes the lock only for `Engine::finish`, so other commands run during the fit. `fit_cancel(jobs, job)` sets the flag. The frontend passes a `Channel` whose `onmessage` receives the events. Sources: Tauri 2, "Calling the Frontend from Rust" (page updated 2026-09-09: channels are "designed to be fast and deliver ordered data", the event system is not meant for high throughput) and "Calling Rust from the Frontend" (updated 2026-06-08: async commands, managed state, channels for streaming); both read 2026-11-07. The capability file lists `fit_start` and `fit_cancel` and nothing else new.

**Web Worker** (browser, placement B). Page to worker: `{type:"start", request}` and `{type:"cancel", job}`, plain objects through `postMessage`, which accepts anything the structured clone algorithm handles (MDN, `Worker.postMessage`, last modified 2026-04-13; `Worker.terminate()`, last modified 2024-07-26, read 2026-11-07). Worker to page: `{type:"event", event}` with the events of section 1, in order. The loop is the one of section 3.

**MCP** (Apothicaire). The MCP revision read is `2026-07-28` (the `latest` path of modelcontextprotocol.io, pages `basic/patterns/progress` and `basic/utilities/cancellation`, read 2026-11-07). Here the request is the job: no `fit.start` tool is needed, the JSON-RPC request id plays the job id. Apothicaire calls the `fit.run` tool with `_meta.progressToken`; `caladrius-mcp` runs `FitRun` and sends `notifications/progress` with `progress` = the iteration (must increase on each notification, as `iteration` does), `total` omitted (it is optional), and `message` = a sentence such as `iteration 7, ofv 1234.5` (the spec gives no field for parameters, so they are not carried). A client cancels with `notifications/cancelled {requestId, reason}` on stdio, or by closing the response stream on Streamable HTTP; the server stops at the next iteration boundary and, per the spec, sends no response for the cancelled request (the `cancelled` sentence is then only logged). A cancel that arrives after the end is ignored, as the spec allows. `caladrius-mcp` must therefore read its input while a fit runs (a reader thread or task: native only). Clients may reset their timeout on progress but keep a maximum (same page).

## 5. What is in, what is out

- **Several jobs at once: out.** One running job per bridge; a second `fit.start` is refused with a sentence naming the running job (the same guard as for mutating commands, section 1). Reasons: one history with a defined order, one wasm memory, one worker. The `job` field is in every event so lifting this later breaks no client.
- **Resuming after a crash: out.** A job that did not end has no entry in the history and no seal; it is lost, and the user starts it again. `project.replay` only re-runs finished fits. Because `FitRun` is a value, saving it is possible later, but a format would have to be sealed and versioned first: not specified here.
- **Partial answers: out as answers.** A cancelled or failed job returns no sealed result. `last` and `trace` in `cancelled` are records for the screen only, never estimates: the trace is carried because FIT-OUT-11 says it is never discarded (orchestrator, 2026-11-07), so `Outcome::Cancelled` holds the trace as well as `after_iteration`.
- **Progress throttling:** the host may drop `progress` events but never the terminal one; the engine does not throttle (it has no clock).
