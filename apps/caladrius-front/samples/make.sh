#!/bin/sh
# The pk3 sample of the start screen: three subjects simulated by the engine (`model.simulate`,
# pk3.iv_bolus, 100 mg), then a fixed multiplicative perturbation (+4 %, -3 %, +2 %, -5 %, ... by row)
# written to four significant digits, so that a fit does not land on an exact zero residual.
# Run from the repository root:  sh apps/caladrius-front/samples/make.sh > apps/caladrius-front/samples/pk3_iv_bolus.csv
set -eu
CLI=${CLI:-cargo run -q -p caladrius-cli --}
TIMES='[0.083,0.25,0.5,1,2,4,6,8,12,24,36,48,72,96]'
echo "Subject,Time (h),Conc (mg/L),Dose (mg)"
sim() {
  $CLI model.simulate --param model=pk3.iv_bolus --param dose=100 --param "params=$2" --param "times=$TIMES" --format csv |
    awk -v s="$1" -F, 'BEGIN { split("1.04 0.97 1.02 0.95 1.03 0.98 1.05 0.96", f, " ") }
      /^#/ || /^time/ { next }
      { i++; printf "%s,%s,%.4g,100\n", s, $1, $2 * f[(i - 1) % 8 + 1] }'
}
sim A '{"cl":2,"vc":10,"q2":5,"vp2":20,"q3":0.5,"vp3":40}'
sim B '{"cl":3,"vc":12,"q2":4,"vp2":15,"q3":0.4,"vp3":60}'
sim C '{"cl":1.5,"vc":8,"q2":6,"vp2":25,"q3":0.6,"vp3":30}'
