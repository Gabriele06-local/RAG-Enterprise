- **A backup archive is now known to be complete, when it is written and
  when it is read.** `pack_tar_gz` finalised the tar but left the gzip stream
  to `Drop`, where flate2 discards the result: a volume that filled during the
  last few kilobytes produced a truncated archive that still decompressed to a
  plausible tar prefix, and the run reported success. The finalisation is now
  awaited and the bytes flushed. Reading one back had the matching gap — the
  tar end-of-archive marker is not the end of the gzip stream, so the CRC
  trailer was never consulted and a truncated archive restored as though it
  were whole. Both sides now check, and a test reads back what it just wrote.
