- **A Qdrant that refuses a snapshot now says so.** Neither the snapshot
  request nor its download checked the HTTP status, so a 404 for a missing
  collection, a 401 for a wrong api-key or a 500 had its body handed straight
  to the JSON parser, which rejected it — leaving `error decoding response
  body: missing field 'result'` as the only account of a failed backup, with
  the status nowhere in it. The status is now checked before the body is
  parsed, as the restore path in the same file already did.
