- **A misspelt `--bench` no longer starts the server instead.** With no
  path after the flag, `parse_args` returns `None` and the run fell through
  to the normal startup: the benchmark quietly became a full server, with a
  database, a listener on port 8000 and the frontend — and no report, and
  nothing in the log to explain its absence. A flag in the path position
  (`--bench --bench-query "q"`) was taken for a file to benchmark, so a
  typo sent the engine looking for a document named `--bench-query`. Both
  now say what is missing and stop, before anything is provisioned. A
  `--bench` with a real path is unaffected.
