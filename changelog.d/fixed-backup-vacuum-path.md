- **A backup works from a directory whose path contains an apostrophe.**
  `VACUUM INTO` takes a string literal and SQLite has no backslash escape, so
  an unescaped `'` in the path ended the literal there and the statement
  failed. Every backup — the 02:00 one and the admin button alike — was
  therefore impossible from a directory like `C:\Users\O'Neill\…` or an
  iCloud/OneDrive folder with a quote in it, with nothing in the message
  saying why. The path is now escaped exactly as the restore path already
  was.
