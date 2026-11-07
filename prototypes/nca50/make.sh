#!/usr/bin/env sh
# Makes prototypes/nca50/nca50.csv: 50 subjects of 20 points for the drag measure of iteration 2 (T-130).
# One-compartment oral model through `caladrius-cli model.simulate`; the parameters of subject i are a fixed
# formula of i, so the file is the same on every run. Concentrations are kept to 4 significant digits.
# Usage: CLI=path/to/caladrius-cli sh prototypes/nca50/make.sh > prototypes/nca50/nca50.csv
set -eu
CLI=${CLI:-caladrius-cli}
TIMES='[0,0.25,0.5,0.75,1,1.5,2,3,4,5,6,8,10,12,16,20,24,30,36,48]'
echo "subject,time,conc,dose"
i=1
while [ "$i" -le 50 ]; do
  v=$(awk -v i="$i" 'BEGIN { printf "%.3f", 15 + (i % 7) * 2.5 }')
  cl=$(awk -v i="$i" 'BEGIN { printf "%.3f", 1.5 + (i % 11) * 0.35 }')
  ka=$(awk -v i="$i" 'BEGIN { printf "%.3f", 0.6 + (i % 5) * 0.4 }')
  "$CLI" model.simulate --param model=pk1.oral_1 --param dose=100 \
    --param params.v="$v" --param params.cl="$cl" --param params.ka="$ka" \
    --param "times=$TIMES" --format csv --no-seal |
    awk -F, -v s="$i" 'NR > 1 { printf "%s,%s,%.4g,100\n", s, $1, $2 }'
  i=$((i + 1))
done
