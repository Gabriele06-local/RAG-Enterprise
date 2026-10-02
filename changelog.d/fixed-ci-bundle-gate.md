- **The check that the committed bundle matches the source now sees added
  files.** It compared with `git diff`, which reports nothing for an untracked
  file, so a `frontend/dist` that gained an asset without losing one passed
  with a bundle no build produces. It now stages the directory and compares
  the index.
