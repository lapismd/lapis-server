# Hosted PWA verification

| ID          | Status      | Evidence                                                                                                                                              |
| ----------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| SRV-HST-025 | In progress | The built Rust Chromium journey checks transient loss, an icon-only right status item, popover focus and reconnect.                                   |
| SRV-HST-026 | In progress | Chromium loads the manifest and uses the installed worker to reload the offline shell. Staging HTTPS and installed-device qualification are pending.  |
| SRV-HST-027 | In progress | Chromium preserves a local draft through an offline reload. IndexedDB tests cover conflict preservation, scope changes and clearing local copies.     |
| SRV-HST-028 | In progress | Chromium verifies a guarded draft save after reconnect and return to the same note. Unit cases cover precondition conflicts and lost acknowledgement. |
