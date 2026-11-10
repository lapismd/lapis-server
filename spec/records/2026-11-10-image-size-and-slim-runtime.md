# Image size and slim runtime qualification

The local public ARM64 image `45f4c1b` has Docker unpacked size
2,373,487,603 bytes. Image history assigns 1.91 GB to the Homebrew prefix copy,
79.7 MB to the executable, 59.1 MB to the static application, and another
139 MB to a later recursive chmod. Inside the running container, Homebrew's
Caskroom uses 1,265 MiB, Cellar 357 MiB and Homebrew itself 219 MiB.
Registry transfer sizes are compressed and are not these unpacked totals.

The executable links glibc (`libc.so.6`, `libm.so.6`, `libgcc_s.so.1` and the
AArch64 glibc loader). Alpine is therefore not a base-only replacement.
The full image retains its supported Homebrew and four agent CLI contract;
the companion slim image omits that distribution and keeps Ubuntu's ABI.
Read-only COPY modes remove the duplicate application permissions layer.

The new `scripts/test-image-slim.sh` fails its size bound against the old image
(one expected failure). A local packaging-only build reuses the old public
image's ARM64 executable and static files in the exact new runtime stage. It
measures 248,443,690 bytes and passes startup/readiness, UID/GID, application
read-only permissions, writable data, omitted tooling and clear rejection of
requested Homebrew installs. The test uses only the local `desktop-linux`
context and removes only its disposable fixture/container/anonymous volume.
Initial fixture failures for missing origin and invalid insecure-cookie origin
were corrected to use the existing `http://localhost` authentication contract.

Source specification, ownership audit, formatting and shell syntax checks pass.
This receipt proves packaging, not a fresh source-built image or the Notes
startup repair. PR CI builds the actual source image and runs the same smoke
on AMD64 and ARM64 before merge. Public tags use the normal tag plus `-slim`;
source builds select `runtime-slim`. The default full tag remains available.
