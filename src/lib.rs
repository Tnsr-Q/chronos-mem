//! chronos-mem: causal memory analysis framework.

pub mod buffer;
pub mod context;
pub mod mesh;
pub mod rubric;
pub mod session;
pub mod voxel;

// Re-export commonly used types.
pub use rubric::{AgentState, LiveProbe, PostMortemRubric, RubricId, RubricRegistry, Score};
pub use voxel::{AtomicOp, MemcpyKind, Operation, Voxel, MAX_RUBRICS_PER_VOXEL, NULL_ADDR};
