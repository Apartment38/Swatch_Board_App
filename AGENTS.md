# Repository instructions for agents

## Commits

- Commit completed work at logical checkpoints instead of accumulating unrelated changes in one commit.
- A logical checkpoint is a coherent, reviewable change whose relevant verification passes and that does not depend on unfinished follow-up work.
- Keep each commit focused. Do not mix unrelated refactors, formatting, generated artifacts, or documentation changes except when they are required by the same change.
- Before committing, include every intended source, test, documentation, and evidence update for that checkpoint. Do not commit temporary files, local configuration, build output, credentials, or unfinished scaffolding.
- Use a concise imperative commit subject that describes the completed behavior.
- Do not commit failing or unverified work unless the user explicitly requests a checkpoint commit; identify that state clearly in the commit message.
- Preserve existing history: do not amend, squash, rebase, reset, or force-push unless the user explicitly requests it.
- Never push commits unless the user explicitly requests a push.
