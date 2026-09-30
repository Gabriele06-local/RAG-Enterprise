- **A backup no longer leaves its snapshot behind on the Qdrant host.**
  Qdrant keeps a copy of every snapshot it creates, and nothing here deleted
  it — the retention quota prunes local archives only, and the only snapshot
  endpoints this code used were the request and the download. So every daily
  backup added a collection-sized snapshot to the Qdrant host, growing
  without bound until that volume filled; and then the snapshot request began
  failing, which aborts the very backup that exists to protect against exactly
  that. Our copy is now verified and the host's is dropped, in that order, so
  a failed verification still leaves the server-side copy for a retry.
