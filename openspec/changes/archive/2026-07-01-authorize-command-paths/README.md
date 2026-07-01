# authorize-command-paths

Stop the commit and artifact commands from trusting caller-supplied repository
and workspace paths. `commit_*` reads any `.git` on the host and `read_artifact`
/ archive reads reach any `openspec/`-shaped directory, because no command checks
the path against the registry. Add a registry-membership guard at the shared
`AppService` boundary and route the divergent desktop commands through it so both
the desktop and web transports enforce the same allowlist.
