- **A source citation can be opened again.** The filename under an answer
  was a plain link to the download endpoint, and a link click sends no
  `Authorization` header — while the API authenticates on
  `Authorization: Bearer` only, with no session cookie to fall back on. So
  clicking the document an answer came from navigated to the endpoint, got a
  401, and handed the user a JSON error body named after the file. Citations
  now fetch the original with the token and save it, under the name the
  server stored.
