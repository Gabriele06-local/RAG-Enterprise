- **A generation that fails mid-stream is no longer stored as a
  complete answer.** eullm reports a failure part-way through by writing an
  `error` object on its own line of the NDJSON stream, which is not a chunk
  and so did not parse. It was skipped like any other unrecognised line,
  which ended the stream *cleanly* — and a cleanly-ended stream is persisted
  as the model's reply, so a context-length overflow or an OOM halfway
  through left the user with a silently truncated answer that came back as
  authoritative on every later reload, and was replayed into the prompt as
  history. Such a line now ends the stream the way a severed connection
  does, so the partial text is shown but not saved. Lines that are not a
  recognised error are still skipped, so a newer eullm cannot cut a good
  answer short.
