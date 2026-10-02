- **Verification no longer reads a file outside the archive.** The manifest
  inside a backup names its members, and those names were joined onto the
  unpacked directory without checking what they were — an absolute path makes
  `join` throw the base away. A hand-delivered archive could therefore have
  the restore hash, and report on, any file the service can open; the digest
  appears in the mismatch message. The name is now held to being a plain file
  name wherever it is first consumed, the same rule the restore already
  applied to the same two names, and the rule is stated once and tested once.
