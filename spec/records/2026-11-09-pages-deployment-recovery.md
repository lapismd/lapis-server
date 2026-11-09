# Pages deployment recovery

## Failure and owning boundary

[Run 42714528083, deploy job 129493410224](https://github.com/lapismd/lapis-server/actions/runs/42714528083/job/129493410224)
built and uploaded Storybook successfully at `45f4c1b`. GitHub accepted the Pages
deployment, reported `syncing_files` repeatedly, then the runner cancelled the
job at its ten-minute deadline. There was no build or content-validation error.
The preceding run `42702428761` at `15d4040c` had the same failure. These are two
cancelled deployment attempts, not failed application tests.

The uploaded ZIP was 19,354,298 bytes, comparable to the last successful build's
19,355,788 bytes. Inspection of its tar found 332 entries, 66,519,678 content
bytes, no symbolic/hard links or special members, and all three expected
Storybook entrypoints under `storybook/`. GitHub's public status did not identify
an incident during these deployments. The evidence locates the stall in the
remote Pages synchronization stage; its internal cause is not observable here.

The exact deploy job was retried using its existing artifact. Merging the fully
passing hosted-content PR then cancelled that retry through the workflow's
`cancel-in-progress: true` policy. That cancellation is distinct from the two
original deadline failures. The newer main run owns the updated deployment.

## Change and acceptance review

SRV-GOV-001 retains the Storybook URL, artifact layout, release ownership and
functional gates. The Pages concurrency group now lets an active run finish.
Newer pushes can replace a pending run instead of aborting active deployment.

The deploy job has 25 minutes for up to two attempts. The official
[`deploy-pages` v5 implementation](https://github.com/actions/deploy-pages/blob/v5/src/internal/deployment.js)
caps each attempt at ten minutes and cancels an unsuccessful deployment. The
outer job previously expired before the action could report its own timeout;
increasing the action's timeout input would still be clamped to ten minutes. The second
attempt uses the same artifact, runs only after first-attempt failure and never
after workflow cancellation. It does not suppress errors: a failed retry fails
the job. The environment URL uses the retry output when present.

Validation: checksum-verified actionlint 1.7.12, formatting, specification checks
and the existing configuration preflight. Product code, dependencies, UI and
artifact construction are unchanged. Real GitHub Pages recovery remains a
deployment check on main; local validation cannot establish service recovery.
