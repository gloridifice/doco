# Project instructions

<!-- DOCO:START -->
<!-- doco:entry template=v2 -->
## Doco

For current project documentation and explicitly tracked changes, use the
`doco` skill. Read `.agents/skills/doco/SKILL.md` before creating, executing, completing,
or archiving a change. Perform only the requested phase.
A doco change package is optional for implementation and is not an
implementation-history mechanism: archiving retains only proposal.md. Routine
behavior fixes and implementation-detail changes may proceed without creating
a doco change unless the user explicitly requests tracking. Use Git, pull
requests, or release notes for implementation history. Regardless of tracking,
update current architecture and specs when their documented facts or contracts
change.
Use the doco library nearest to the working directory, or select one with
`doco --root <library-root> ...`. Child libraries keep their own documents and
changes but reuse this installed skill; do not generate child skill or Agent
entry files. Resolve the skill path above from this entry's directory and
`doco/` paths from the selected library. Parent documents describe shared
constraints; child documents describe local facts without automatic overrides.
`doco list` includes all project libraries, current first; write commands and
change IDs remain local to the selected library.
Start from its `doco/architecture.md` and relevant current specs and decisions.
When implementing a selected tracked change, use its proposal and any present
design and task files. Treat completed changes, archived changes, and
`doco/tmp/` as non-current material; consult them only when explicitly needed.
<!-- DOCO:END -->
