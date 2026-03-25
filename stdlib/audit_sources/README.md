# stdlib audit sources

This directory contains the Move source files kept for audit, review, and source-level comparison.

These `.move` files are not the artifacts used by the current Nexus Move runtime path.

For actual execution and embedding, the system uses compiled Move bytecode artifacts (`.mv` files), not the source files in this directory.

In practice, treat this directory as:

- audit reference material
- source-level inspection input
- a human-readable companion to the compiled stdlib artifacts

Do not assume that changing a file in this directory will affect runtime behavior unless the corresponding compilation and artifact refresh steps are performed for the real `.mv` outputs.