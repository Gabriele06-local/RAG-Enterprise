- **Searches go through Qdrant's Query API.** The Search API they used is
  deprecated, and Qdrant's 1.19 release notes announce its removal — 1.19.1
  still answers it, but the next update may not. Same results, nothing to
  do on upgrade; checked against the bundled Qdrant 1.18.2 and against
  1.19.1.
