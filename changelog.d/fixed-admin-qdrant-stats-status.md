- **A refused Qdrant reply is no longer shown as the collection's state.**
  The stats handler passed Qdrant's body to the browser without checking the
  status, and a Qdrant error is valid JSON — so a missing collection, a wrong
  api-key or a typo in `QDRANT__COLLECTION` rendered in the admin panel as
  `0 points`, `0 vectors`: a full collection looking empty. It is now a
  failure, and the status and body go to the log where the rest of the
  codebase puts a 5xx's detail.
