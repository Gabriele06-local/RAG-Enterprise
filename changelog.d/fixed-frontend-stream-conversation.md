- **Switching conversation mid-answer no longer writes into the wrong
  thread.** The conversation list stays clickable while a reply streams in,
  and every arriving token was appended to the last message on screen. So
  leaving a conversation mid-answer spliced the rest of the answer into
  the one opened instead, and coming back before it finished appended it
  to the question itself, since the reloaded list has the question but not
  yet the answer. The stream now stops writing to the screen as soon as the
  conversation changes; if you are back on it when the answer is complete,
  the conversation is reloaded with the full answer. First reported, and
  fixed for the switch away, in #86.
