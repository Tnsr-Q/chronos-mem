
chronos-mem/
├── Cargo.toml                 # unchanged from Deliverable 9
└── src/
    ├── lib.rs                 # module declarations + public re-exports
    ├── buffer.rs # ┐    ├── context.rs             # ├ implemented as stubs in this first commit
    ├── mesh.rs                # │
    ├── session.rs             # ┘
    ├── rubric.rs              # ✅ fully revised: LiveProbe, PostMortemRubric, Score, Registry
    ├── voxel.rs               # ✅ fully revised: Voxel (no lifetime), Operation, atomic/memcpy kinds
    └── cli/
        ├── main.rs            # CLI entry point
        └── commands/
            ├── init.rs        # session initialization
            ├── run.rs         # capture / run workload
            ├── analyze.rs     # post-mortem rubric reports
            └── sessions.rs    # list/manage existing sessions
```

**Notes:**
- The top-level `src/` modules all exist as files now, with `rubric.rs` and `voxel.rs` containing the first-commit implementation.
- `buffer.rs`, `context.rs`, `mesh.rs`, and `session.rs` are stubs (placeholders with module docs) so the crate builds.
- The `cli/` subtree is part of the planned project structure but has not been implemented yet.
