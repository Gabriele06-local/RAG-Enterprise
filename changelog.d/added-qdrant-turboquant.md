- **The vectors can take 8 or 16 times less RAM: `QDRANT__QUANTIZATION`.**
  With `turbo4`, Qdrant keeps a TurboQuant copy of every vector in RAM —
  520 bytes instead of 4 KiB at 1024 dimensions — and moves the originals to
  disk. Each search picks its candidates on the compressed copies and
  re-scores twice as many as it returns on the originals; `turbo2` halves
  the RAM again and re-scores four times as many. The results are nearly
  always those of a full-precision search, not always: a chunk the
  compression ranks below that shortlist is never re-scored, so it is
  missed — most likely one nearly tied with others near the bottom of the
  list. On a synthetic test of 30,000 vectors, `turbo4` kept all of the top
  15 results and `turbo2` 99% of them; try it with `--bench` on your own
  documents before relying on it. `off`, the default, keeps every vector in
  RAM as before. The setting is applied to the existing collection at
  startup, and undone the same way; Qdrant re-encodes the vectors in the
  background while searches go on. Needs Qdrant 1.18 or later, which is
  what the bundle ships.
