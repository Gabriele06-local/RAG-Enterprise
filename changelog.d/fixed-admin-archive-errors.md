- **A bad backup archive is now reported as a bad request, with its
  reason.** Picking the wrong archive, restoring one whose digest does not
  match, one missing a member, or one taken from a collection this
  installation is not configured for, was answered `500` — and a 5xx's message
  is deliberately withheld, so the admin was told "internal server error" and
  nothing about which archive or why. The classification now also reads the
  whole error chain, so a `.context()` added in the service cannot quietly
  demote a 4xx into a 5xx, and a test holds every refusal the service can
  produce so a new one has to be classified deliberately.
