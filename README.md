[README.md](https://github.com/user-attachments/files/29858943/README.md)
```text
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


####  Rubric State Management Design

State is removed from the hot path and handled in two ways:

1.  **Stateless `LiveProbe`:** Probes like a basic `ReadWriteRatioProbe` are purely functional (`score_op` is a pure function of the `Operation`).
2.  **Stateful `PostMortemRubric`:** Complex, stateful analysis is deferred to the `analyze` command. The `FalseSharingRubric` is a prime example. During analysis, it can build a `HashMap<CacheLine, Vec<(EpsilonId, Sequence)>>` in memory by iterating through all voxels, allowing it to detect conflicting writes without any runtime overhead or locks.

For the rare case where per-agent state is needed live (e.g., `LocalityProbe`), it is managed by the `KernelContext` itself, not the rubric.

#### 3. Rubric State Management Design

State is removed from the hot path and handled in two ways:

1.  **Stateless `LiveProbe`:** Probes like a basic `ReadWriteRatioProbe` are purely functional (`score_op` is a pure function of the `Operation`).
2.  **Stateful `PostMortemRubric`:** Complex, stateful analysis is deferred to the `analyze` command. The `FalseSharingRubric` is a prime example. During analysis, it can build a `HashMap<CacheLine, Vec<(EpsilonId, Sequence)>>` in memory by iterating through all voxels, allowing it to detect conflicting writes without any runtime overhead or locks.

For the rare case where per-agent state is needed live (e.g., `LocalityProbe`), it is managed by the `KernelContext` itself, not the rubric.

```rust
// In context.rs - KernelContext's internal state
pub(crate) struct AgentState {
    pub last_addr: u64,
    // ... other per-agent state ...
}

// The `score_op` signature is modified to receive this state.
// pub trait LiveProbe {
//    fn score_op(&self, op: &Operation, state: &AgentState) -> Score;
// }
```

#### 4. `tau` Ordering Specification

-   **Semantics:** The global scalar `tau` is **eliminated**. The new temporal coordinate is a vector clock `(epsilon_id, sequence)`. This defines a total order of operations for a single agent (`epsilon`) and a "happens-before" partial order across agents.
-   **Determinism:** This is **fully deterministic**. Given the same program logic, each agent will always generate the same sequence of operations with the same sequence numbers.
-   **Query Semantics:**
    -   `--query slice --epsilon 5 --seq 100:150`: This is now the primary way to slice time. It performs a range query on the temporal index for a specific agent.
    -   `--query slice --time "10.5s:11.0s"`: This is a "best effort" query against `wall_time_ns`, useful for debugging but not for deterministic regression analysis. It scans the `seam_log` CF.

####  RocksDB Schema

-   **Session ID:** All keys are prefixed with a 16-byte `SessionId` to isolate runs.
-   **Column Families:** Three column families are used to create indices for different query patterns.
-   **Key Encoding:** All integer types are encoded as big-endian bytes (`.to_be_bytes()`) to ensure lexicographical order matches numerical order.

1.  **`cf_primary` (Blame & Hotspots):** Optimized for lookup by address.
    -   **Key:** `[SessionId (16B) | Delta Addr (8B) | Epsilon ID (8B) | Sequence (8B)]`
    -   **Value:** `Voxel` struct (bincode serialized).
    -   **Queries:**
        -   `blame --addr X`: Prefix scan for `[SessionId | X]`.
        -   `hotspots`: Full scan on this CF, aggregating scores in memory.

2.  **`cf_temporal` (Per-Agent Timeline & Slice):** Optimized for lookup by agent and time.
    -   **Key:** `[SessionId (16B) | Epsilon ID (8B) | Sequence (8B) | Delta Addr (8B)]`
    -   **Value:** `Voxel` struct.
    -   **Queries:**
        -   `slice --epsilon 5 --seq A:B`: Range scan from `[SessId | 5 | A]` to `[SessId | 5 | B]`.

3.  **`cf_seam_log` (Global Timeline):** A lightweight log of only seam events, ordered by wall-clock time.
    -   **Key:** `[SessionId (16B) | Wall Time ns (8B) | Epsilon ID (8B)]`
    -   **Value:** The `Operation` enum (bincode serialized).
    -   **Queries:**
        -   `timeline`: Full scan of this CF for the given `SessionId`.

-   **Space Amplification:** We store the full `Voxel` twice. `cf_seam_log` is negligible. The expected on-disk size is **~2.1x** the raw size of all generated Voxels.
