#!/usr/bin/env bash
set -euo pipefail

SCRIPT_VERBOSE=false
if [[ ${1:-} == "--verbose" ]]; then
  SCRIPT_VERBOSE=true
  shift
fi

FILE="${1:-nl2gql/sample_queries.txt}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '4')}"

if [[ ! -f "$FILE" ]]; then
  echo "File not found: $FILE" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

tmp_list="$tmp_dir/commands.txt"
awk 'NF && $1 !~ /^#/' "$FILE" >"$tmp_list"

if [[ ! -s "$tmp_list" ]]; then
  echo "No runnable commands found in $FILE" >&2
  exit 1
fi

all_cmds=()
all_logs=()
all_status=()
running_pids=()
running_idx=()

launch_job() {
  local cmd="$1" log="$2" idx="$3"
  if $SCRIPT_VERBOSE; then
    # Stream output to the console while still capturing it for parsing.
    # Use process substitution so the job exit status reflects the command, not tee.
    bash -lc "set -o pipefail; $cmd" > >(tee "$log") 2> >(tee -a "$log" >&2) &
  else
    bash -lc "set -o pipefail; $cmd" >"$log" 2>&1 &
  fi
  running_pids+=("$!")
  running_idx+=("$idx")
}

wait_oldest() {
  local pid="${running_pids[0]}"
  local idx="${running_idx[0]}"
  local status=0
  if wait "$pid"; then
    status=0
  else
    status=$?
  fi
  all_status[$idx]=$status
  running_pids=("${running_pids[@]:1}")
  running_idx=("${running_idx[@]:1}")
}

idx=0
while IFS= read -r cmd; do
  [[ -z "$cmd" ]] && continue
  log="$tmp_dir/job_${idx}.log"
  all_cmds+=("$cmd")
  all_logs+=("$log")
  launch_job "$cmd" "$log" "$idx"
  if [[ ${#running_pids[@]} -ge $JOBS ]]; then
    wait_oldest
  fi
  idx=$((idx + 1))
done <"$tmp_list"

while [[ ${#running_pids[@]} -gt 0 ]]; do
  wait_oldest
done

total_calls=0
total_tokens=0
success_count=0

parse_stat() {
  local label="$1" file="$2"
  awk -v lbl="$label" '$1==lbl {print $2}' "$file" | tail -n1
}

printf "Summary (JOBS=%s)\n" "$JOBS"

count=${#all_cmds[@]}
for i in $(seq 0 $((count - 1))); do
  status=${all_status[$i]:-1}
  log=${all_logs[$i]}
  calls=0
  tokens=0
  if [[ -f "$log" ]]; then
    calls=$(parse_stat "Calls:" "$log")
    tokens=$(parse_stat "Tokens:" "$log")
  fi
  [[ -z "$calls" ]] && calls=0
  [[ -z "$tokens" ]] && tokens=0
  if [[ $status -eq 0 ]]; then
    success_count=$((success_count + 1))
  fi
  total_calls=$((total_calls + calls))
  total_tokens=$((total_tokens + tokens))
  printf "  [%02d] %s | calls=%s | tokens=%s\n" "$((i + 1))" "$( [[ $status -eq 0 ]] && echo "OK" || echo "FAIL($status)" )" "$calls" "$tokens"
done

printf "Successful queries: %d/%d\n" "$success_count" "$count"
printf "Total calls: %d\n" "$total_calls"
printf "Total tokens: %d\n" "$total_tokens"
