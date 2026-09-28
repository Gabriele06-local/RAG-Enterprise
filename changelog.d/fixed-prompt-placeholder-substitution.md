- **A retrieved chunk is no longer rewritten by the prompt template.** The
  prompt was assembled with a chain of `str::replace` calls, each scanning
  what the previous one had produced. Any chunk or stored message containing
  the literal text `{question}` or `{context}` — a template guide, a config
  file, a document *about* this project — had that token replaced by the
  user's question, inside the evidence the model was shown. The evidence
  stopped being what the document says, and the text the user asked about
  appeared in the middle of it. Substitution now happens in a single pass
  over the template, so an inserted value is never scanned again. The prompt
  is byte-for-byte what it was for any document that does not contain the
  tokens.
