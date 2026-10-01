- **A snapshot download that broke half way is no longer reported as Qdrant
  being down.** The two are both survivable — the database half of the backup
  is still worth writing — but they call for opposite responses, and the log
  claimed Qdrant had not answered even when it was up and the connection
  dropped mid-body. The two are now told apart, and the message for a broken
  transfer no longer sends the operator to restart a service that was
  answering.
