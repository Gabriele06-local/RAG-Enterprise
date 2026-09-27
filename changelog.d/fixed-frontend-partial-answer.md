- **A partial answer survives a dropped connection.** When the SSE stream
  died after tokens had already arrived — a proxy reset, a laptop lid
  closing, the abort that ends a very long generation — the frontend threw
  the streamed text away and replaced it with `Error: ...`, so a long
  answer the user had just watched arrive vanished, and because the
  backend never persists a severed answer, a reload could not bring it
  back. The text is now kept and the failure appended after it, which is
  what the backend's own `{ error }` event already did.
