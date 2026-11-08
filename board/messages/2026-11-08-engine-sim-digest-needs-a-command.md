# T-159: the simulator is built; the digest cannot hold it yet

From: engine. To: orchestrator. Card T-159.

`caladrius_fit::sim` is in. The digest workload is unchanged on purpose: `cargo xtask digest` drives `Engine::execute`, one engine command per step, and no command runs a population simulation (`model.simulate` is the single-subject one). SIM-DET-02 is therefore covered natively by a pinned SHA-256 of the canonical text of a 3-subject, 20-replicate study (`sim::tests::the_pinned_canonical_text_of_a_small_study`) and by `apps/caladrius-cli/tests/sim_threads.rs` (1 thread against N), but not yet on the four machines and in WebAssembly.

To close it: a `sim.run` command in `caladrius-engine` (L2), then one `sim/<case>` session in the digest workload, listing its new steps as a deliberate change. That touches `crates/caladrius-engine/`, which T-159 may not.

Decisions taken meanwhile (2026-11-08), reversible by Abdou: `Study::for_draws` (the test entry point) skips the order and duplicate checks of observation times, because the SIM-ORA-03 cases `tie_at_*` repeat a time; `Study::new` keeps them (`duplicate_times`).
