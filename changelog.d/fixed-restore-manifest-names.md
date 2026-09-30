- **A restore applies the members the archive's manifest names, and
  refuses an archive of another collection.** Verification and restore
  disagreed about where the members are: the manifest was hashed under the
  names it gives, then the files were looked for under names rebuilt from the
  current configuration. Rename `QDRANT__COLLECTION`, or move an archive to
  another installation, and the snapshot was verified and then never found —
  the run reported success and only the database came back. The two now agree
  by construction, and an archive whose manifest names a different collection
  is refused before anything is written rather than uploaded into the wrong
  one. A restore that finds no database at all is an error, not a successful
  restore of nothing; the collection name was already in every manifest and
  was read by nothing.
