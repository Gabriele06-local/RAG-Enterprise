- **A document row that was not written is no longer reported as written.**
  The insert used `INSERT OR IGNORE`, but the id is a fresh UUID per upload and
  is the table's only unique column, so there was nothing for it to ignore: it
  could only mask the one failure that must not be masked. An ignored insert
  returns success, the upload is reported as done, and its vectors are already
  in Qdrant — leaving an orphan with no row for the delete path to reach.
  Uploading the same file twice is unaffected: only the id is unique.
