- **Logging out no longer leaves the next login with a disabled
  interface.** The "something is running" flags belonged to the request, not
  to the session, and only that request's own cleanup cleared them. Signing
  out during an answer left the question box and the send button disabled
  until the abandoned stream ended — up to its ten-minute abort — and
  signing out during an upload left the file picker disabled for as long as
  the upload ran, with the previous session's "Processing (OCR → Chunking →
  Embedding)" banner still on screen. Those flags are now cleared on logout,
  and a stream that no longer owns the view stops writing to it instead of
  appending into whatever the next session is looking at.
