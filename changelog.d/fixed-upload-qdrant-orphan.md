- **An upload that cannot record its metadata does not leave its vectors
  behind.** Ingestion wrote the chunks to Qdrant first and the SQLite row
  second, with nothing between them: a failing insert left a document whose
  vectors were still retrieved by every query and still quoted as a source
  pointing at an id that resolves to nothing, while the document list — read
  from SQLite — never showed it. So there was no id to delete them by and no
  way to reach them again short of rebuilding the collection. The vectors are
  now removed again when the insert fails, which is the same invariant
  `purge_document` already states for the delete path — and when the Qdrant
  write itself fails part-way, which on a document of more than 1000 chunks
  left the batches already written behind in the same way.
