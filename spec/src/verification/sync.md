# Lapis Sync verification

The contract is established, but implementation and native/application
qualification remain open. The isolated draft audit is negative evidence only;
it does not qualify a source or consumer boundary.

| ID           | Status      | Evidence                                                                                                                                                                                                |
| ------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SRV-SYNC-001 | In progress | Canonical ownership and exact destination admission are specified. The isolated unsafe draft audit found no qualifying provider selection or profile/vault/root admission; implementation remains open. |
| SRV-SYNC-002 | In progress | Authentication, enrollment, content-key, owner and recovery authority separation is specified. No current Sync source qualification is claimed.                                                         |
| SRV-SYNC-003 | In progress | Owner-authorized endpoint/participant enrollment is specified. The prior draft's unscoped six-digit pairing and replace semantics are retained as negative audit evidence only.                         |
| SRV-SYNC-004 | In progress | Destination durability ordering and retry identity are specified. The prior draft exposed process-relative writes and failed 0/6 destination cases; replacement implementation remains open.            |
| SRV-SYNC-005 | In progress | Profile-private repository isolation, guarded paths and the 64 MiB in-flight target are specified. No filesystem boundary is qualified.                                                                 |
| SRV-SYNC-006 | In progress | Signed dependency, membership and cutoff validation is specified. Server conformance remains open.                                                                                                      |
| SRV-SYNC-007 | In progress | Bounded paging, cursor expiry and empty-side safety are specified. Catch-up implementation remains open.                                                                                                |
| SRV-SYNC-008 | In progress | Continuous enrolled server participation and browser-independent capture are specified. No hosted participant is active.                                                                                |
| SRV-SYNC-009 | In progress | The 20-second quiet/five-minute maximum single-flight checkpoint policy and retained outcomes are specified. Native scheduling remains open.                                                            |
| SRV-SYNC-010 | In progress | Exclusive guarded apply, writer protection and idempotent recovery are specified. Working-vault mutation qualification remains open.                                                                    |
| SRV-SYNC-011 | In progress | Scoped bounded HTTPS endpoints and pre-body authorization are specified. No Sync HTTP destination is exposed.                                                                                           |
| SRV-SYNC-012 | In progress | Independent store/key/server qualification and evidence separation are specified. All product acceptance journeys remain open.                                                                          |
