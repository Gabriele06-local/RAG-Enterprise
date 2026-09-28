- **Switching conversation mid-answer no longer corrupts the thread you
  switched to.** The conversation list and the "new conversation" button
  stayed clickable while a reply streamed in, and every arriving token was
  appended to the last message of whatever was on screen — so the tail of
  one answer was spliced into an unrelated conversation, and a message
  list could grow that the backend never had. The answer itself was always
  filed under the right conversation server-side; only the live view
  misplaced it. Streaming now stops writing once you have moved on, and
  the answer is there when you come back.
