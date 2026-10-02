- **The admin panel's vector listing no longer stops at the first page.**
  It asked Qdrant once for a fixed 10,000 points, but the limit counts chunks,
  not documents, and a document is a few dozen of those. Past it the list simply
  ended — and the sync check below it then reported every document past that
  point as "in SQLite but not in Qdrant", listing healthy, fully-present
  documents for deletion. The scroll is now followed to the end.
