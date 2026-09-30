- **A failed backup no longer leaves its work directory on the backup
  volume.** The directory holding the fresh database copy — and, when Qdrant
  answered, its snapshot too — was removed only on the success path, so six of
  the seven ways a run can fail abandoned a full copy of the database there.
  Nothing reclaimed it: the backup listing only shows `*.tar.gz`, and retention
  only deletes what the listing returned, so a leftover stayed outside the
  quota indefinitely. On a volume filling up because of the leak, the failures
  then fed themselves. The directory is now owned by a guard that removes it on
  every exit, successful or not, and a pack that fails part-way also removes its
  half-written archive, which the listing would otherwise have offered for
  restore.
