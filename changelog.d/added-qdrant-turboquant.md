- **The vectors can take 8 or 16 times less RAM: `QDRANT__QUANTIZATION`.**
  With `turbo4`, Qdrant keeps a TurboQuant copy of every vector in RAM —
  520 bytes instead of 4 KiB at 1024 dimensions — and moves the originals to
  disk. Each search picks its candidates on the compressed copies and
  re-scores the best of them on the originals, so it finds what a
  full-precision search finds. `turbo2` halves the RAM again, for boards
  short of it, and re-scores more candidates to make up for it. `off`, the
  default, keeps every vector in RAM as before. The setting is applied to
  the existing collection at startup, and undone the same way; Qdrant
  re-encodes the vectors in the background while searches go on. Needs
  Qdrant 1.18 or later, which is what the bundle ships.
