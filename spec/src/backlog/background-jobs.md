# Installable plugin background jobs

The agreed initial delivery supports vault-only scheduled jobs in signed plugin
bundles on Desktop and Server. One designated installation runs each job. Desktop
keeps enabled work alive while its process runs, including after the last vault
window closes. Server work survives browser detach. Tasks ships an opt-in daily
overdue digest using its existing Review query.

## Remaining qualification

The native engine, private journals, installed discovery, scoped broker, guarded
writes, notification history and explicit runner handoff are implemented and
[source-qualified](../../records/2026-11-24-plugin-background-runtime.md).

- Complete Notes publication and registry-only public API adoption.
- Finish the real authenticated Server settings, browser detach and cold-restart
  acceptance, plus the complete hosted and interaction/accessibility lanes.
- Pin Desktop to the reviewed native revision, qualify native metadata as the
  default, last-window retention, cold resume, explicit vault close and Quit drain.
- Qualify the installable Tasks handler and signed release in both actual hosts.
- Retain source, consumer, published package and deployed-image evidence separately.

Job declarations are disabled until the owner approves a runner and its grants.
Interval, daily and manual invocations share one run admission boundary. A job
has at most one active run. Pending writes after process loss require inspection
before another invocation. Run records retain bounded history and actionable
failure status.

The plan defaults are explicit runner handoff, permission-scoped note-property
writes and optional local Desktop notifications. Outbound HTTPS, credentials,
push delivery, event triggers, long-lived subscriptions and execution after
Desktop Quit remain future work.
