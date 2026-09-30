- **A backup run is serialized as a whole, not only its retention pass.**
  The lock was taken inside the pass, so it protected list-and-delete against
  another pass but not against the window between a run's write and its own
  pass. A competing run could take the fresh archive in that window, leaving
  the first run returning a path to a file that had just been deleted — the
  admin answering `ok: true` for a backup that is not there. The lock is now
  taken before the archive is written and the guard handed to the pass, so
  list-and-delete can never overlap another run's write.
