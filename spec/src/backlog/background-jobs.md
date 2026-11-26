# Installable plugin background jobs

The agreed initial delivery supports vault-only scheduled jobs in signed plugin
bundles on Desktop and Server. One designated installation runs each job. Desktop
keeps enabled work alive while its process runs, including after the last vault
window closes. Server work survives browser detach. Tasks ships an opt-in daily
overdue digest using its existing Review query.

## Completed v1 qualification

The native engine, private journals, installed discovery, scoped broker, guarded
writes, notification history and explicit runner handoff are implemented and
[source-qualified](../../records/2026-11-24-plugin-background-runtime.md).

Hosted controls, registry-only dependencies, Desktop native defaults and lifecycle,
and the signed released Tasks handler in both actual hosts are complete. The
[closure receipt](../../records/2026-11-26-background-job-verification-closure.md)
keeps source, consumer, package and deployed-registry evidence distinct.

## Remaining work

- Complete actual macOS system-alert permission/delivery acceptance in Desktop.
- Qualify Server2.10.0 image/CLI and Desktop rc.5 distribution independently.
- Implement explicit scoped network and opaque credential capabilities in the
  shared native engine and qualify both hosts. Use Beancount's native bank-service
  boundary: never expose stored bank keys to the guest, and retain reviewed,
  revision-checked ledger imports in the owning service.

Job declarations are disabled until the owner approves a runner and its grants.
Interval, daily and manual invocations share one run admission boundary. A job
has at most one active run. Pending writes after process loss require inspection
before another invocation. Run records retain bounded history and actionable
failure status.

The plan defaults are explicit runner handoff, permission-scoped note-property
writes and optional local Desktop notifications. Outbound HTTPS and credentials
are the next authorized extension. Push delivery, event triggers, long-lived
subscriptions and execution after Desktop Quit remain future work.
