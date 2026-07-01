# harden-git-ref-args

Close a critical arbitrary-file-write hole: a caller-supplied commit ref (`sha`)
flows positionally into `git show` / `git diff-tree` with no option terminator
and no validation, so a ref like `--output=<path>` makes git write to an
attacker-chosen file. Validate refs as object ids and pass them after
`--end-of-options`, at the git sink so every frontend and transport is covered.
