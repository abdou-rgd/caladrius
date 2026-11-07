# NCA screen in egui 0.37 (T-135)

One command opens the screen on `oracle/data/edge_blq.csv`, subject 3, extravascular, dose 100:

```
cargo run --release -p caladrius-proto-egui
```

`CALADRIUS_DARK=1` starts it in the dark theme. `CALADRIUS_MEASURE=<file>` runs the measure of the wave-3 brief (100 drags on `prototypes/nca50/nca50.csv`) and exits; the JSON is also printed on one line starting with `CALADRIUS_MEASURE `, for a shell to keep when the program may not write a file (see `measure-local.json`).

The four screens in `screens/` (light and dark, fresh and stale, 1440 by 900) come from `cargo run -p caladrius-proto-egui --example snapshot`. The keys are listed in the card `board/tasks/T-135.md`.
