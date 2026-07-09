//! Voxel and operation definitions for chronos-mem.

use serde::{Deserialize, Serialize};

use crate::rubric::{RubricId, Score, NONE_RUBRIC_ID};

/// Maximum number of rubric scores stored inline in a single voxel.
pub const MAX_RUBRICS_PER_VOXEL: usize = 8;

/// Sentinel address for operations that are not tied to a logical memory address.
/// Logical address zero is reserved by convention.
pub const NULL_ADDR: u64 = 0;

/// A single instrumented operation in the session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Voxel {
    /// Wall-clock time in nanoseconds; best-effort global ordering.
    pub wall_time_ns: u64,
    /// Execution agent ID (epsilon).
    pub epsilon_id: u64,
    /// Per-agent monotonic sequence number.
    pub sequence: u64,
    /// Logical memory address; `NULL_ADDR` for address-less operations.
    pub delta_addr: u64,
    /// The operation payload.
    pub operation: Operation,
    /// Fixed-size, allocation-free score storage.
    pub scores: [(RubricId, Score); MAX_RUBRICS_PER_VOXEL],
    /// Optional16-byte value snapshot.
    pub value_snapshot: Option<[u8; 16]>,
}

impl Voxel {
    /// Build a blank voxel with all scores initialized to `Score::None`.
    pub fn new(
        wall_time_ns: u64,
        epsilon_id: u64,
        sequence: u64,
        delta_addr: u64,
        operation: Operation,
    ) -> Self {
        Self {
            wall_time_ns,
            epsilon_id,
            sequence,
            delta_addr,
            operation,
            scores: [(NONE_RUBRIC_ID, Score::None); MAX_RUBRICS_PER_VOXEL],
            value_snapshot: None,
        }
    }
}

/// All possible operation types that can be logged as a voxel.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Atomic(AtomicOp),

    // Seam / control-plane events
    SeamCrossLaunch {
        kernel_id: u32,
        stream_id: u16,
        grid_dim: u64,
        block_dim: u64,
    },
    SeamCrossSync {
        stream_id: u16,
        kernel_id: u32,
    },
    SeamCrossAlloc {
        size: u64,
    },
    SeamCrossMemcpy {
        kind: MemcpyKind,
        len: u64,
    },
}

impl Operation {
    /// True for memory operations that have a meaningful `delta_addr`.
    pub fn is_memory_op(&self) -> bool {
        matches!(self, Self::Read | Self::Write | Self::Atomic(_))
    }

    /// True for control-plane seam events.
    pub fn is_seam(&self) -> bool {
        matches!(
            self,
            Self::SeamCrossLaunch { .. }
                | Self::SeamCrossSync { .. }
                | Self::SeamCrossAlloc { .. }
                | Self::SeamCrossMemcpy { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AtomicOp {
    Add,
    Subtract,
    Min,
    Max,
    And,
    Or,
    Xor,
    Exchange,
    CompareExchange,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemcpyKind {
    HostToDevice,
    DeviceToHost,
    DeviceToDevice,
    HostToHost,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rubric::Score;

    #[test]
    fn voxel_is_copy_and_serializable() {
        let v = Voxel::new(1, 2, 3,4, Operation::Read);
        let bytes = bincode::serialize(&v).expect("voxel should serialize");
        let restored: Voxel = bincode::deserialize(&bytes).expect("voxel should deserialize");
        assert_eq!(v.wall_time_ns, restored.wall_time_ns);
 assert_eq!(v.epsilon_id, restored.epsilon_id);
        assert_eq!(v.scores, [(NONE_RUBRIC_ID, Score::None); MAX_RUBRICS_PER_VOXEL]);
    }

    #[test]
    fn operation_classify() {
        assert!(Operation::Read.is_memory_op());
        assert!(Operation::SeamCrossLaunch {
            kernel_id: 0,
            stream_id: 0,
            grid_dim: 1,
            block_dim: 1
        }
        .is_seam());
    }
}
