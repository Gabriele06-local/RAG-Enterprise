- **A restore connection that could not be put back is now closed instead of
  pooled.** The three statements that reset it discarded their errors. If the
  rollback failed — the same `SQLITE_BUSY` or `SQLITE_FULL` that made the copy
  fail — the write transaction stayed open, the detach could not succeed while
  one was, and turning foreign keys back on is a silent no-op inside a
  transaction. The connection then went back to the pool holding the write lock
  with foreign-key enforcement off, which is precisely what those statements
  exist to prevent, and every later user of it inherited both with nothing to
  show why. Each step is now checked, and a connection that could not be reset
  is closed so the pool discards it.
