- **A conversation title is never cut in half a character.** The automatic
  title truncated with `substring(0, 50)`, which counts UTF-16 code units, so
  a question containing a character outside the basic plane — an emoji, rarer
  CJK, some Indic conjuncts, two units each — was cut between the halves at
  position 49 or 50, and the stored title carried a lone surrogate that every
  renderer draws as `�`. The cut is now on a code point.
