//! Rubric traits, score types, and the live-probe registry.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::voxel::{Operation, Voxel, MAX_RUBRICS_PER_VOXEL};

/// Opaque identifier for an execution agent (CPU thread, stream, etc.).
pub type EpsilonId = u64;

/// Stable identifier for a registered rubric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RubricId(pub u8);

/// Reserved rubric ID used to mark unused score slots in a voxel.
pub const NONE_RUBRIC_ID: RubricId = RubricId(0);

/// Fixed-size score container.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Score {
    None,
    Binary(bool),
    Scalar(f32),
    Categorical(u8),
}

/// Cheap per-agent derivative state available on the hot path.
///
/// This state is updated for every instrumented API call, even when sampling
/// skips voxel creation, so probes like `LocalityProbe` remain consistent.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentState {
    /// Last logical address touched by this agent.
    pub last_addr: u64,
    /// Count of logged reads.
    pub read_count: u64,
    /// Count of logged writes.
    pub write_count: u64,
}

/// A live probe scores memory operations on the hot path.
pub trait LiveProbe: Send + Sync {
    /// Stable rubric identifier (must be unique within a registry).
    fn id(&self) -> RubricId;
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Score a single operation.
    ///
    /// Probes are stateless in ownership: any per-agent mutable state must
    /// live in `AgentState`, which is managed by `KernelContext`.
    fn score_op(&self, op: &Operation, state: &AgentState) -> Score;
}

/// A post-mortem rubric receives all voxels and produces a textual report.
pub trait PostMortemRubric {
    fn name(&self) -> &'static str;
    fn analyze(&self, voxels: &[Voxel]) -> String;
}

/// Errors returned by [`RubricRegistry`].
#[derive(Debug, Clone, PartialEq)]
pub enum RegistryError {
    TooManyProbes { found: usize, max: usize },
    DuplicateId(RubricId),
    ReservedId(RubricId),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::TooManyProbes { found, max } => {
                write!(f, "too many live probes registered: {} > {}", found, max)
            }
            RegistryError::DuplicateId(id) => write!(f, "duplicate rubric id: {:?}", id),
            RegistryError::ReservedId(id) => {
                write!(f, "rubric id {:?} is reserved for internal use", id)
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Registry of live probes used by a session.
pub struct RubricRegistry {
    probes: Vec<Arc<dyn LiveProbe>>,
    by_id: HashMap<RubricId, Arc<dyn LiveProbe>>,
}

impl RubricRegistry {
    /// Hard limit on simultaneously active live probes.
    pub const MAX_LIVE_PROBES: usize = MAX_RUBRICS_PER_VOXEL;

    pub fn new() -> Self {
        Self {
            probes: Vec::new(),
            by_id: HashMap::new(),
        }
    }

    /// Register a live probe. Fails if the ID is duplicated, reserved, or the /// probe limit is exceeded.
    pub fn register<P>(&mut self, probe: P) -> Result<(), RegistryError>
    where
        P: LiveProbe + 'static,
    {
        let id = probe.id();
        if id == NONE_RUBRIC_ID {
            return Err(RegistryError::ReservedId(id));
        }
        if self.by_id.contains_key(&id) {
            return Err(RegistryError::DuplicateId(id));
        }
        if self.probes.len() >= Self::MAX_LIVE_PROBES {
            return Err(RegistryError::TooManyProbes {
                found: self.probes.len() + 1,
                max: Self::MAX_LIVE_PROBES,
            });
        }

        let arc = Arc::new(probe);
        self.probes.push(arc.clone());
        self.by_id.insert(id, arc);
        Ok(())
    }

    /// Lookup a probe by its rubric ID.
    pub fn get(&self, id: RubricId) -> Option<&Arc<dyn LiveProbe>> {
        self.by_id.get(&id)
    }

    /// Iterate probes in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn LiveProbe>> + '_ {
        self.probes.iter()
    }

    pub fn len(&self) -> usize {
        self.probes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }
}

impl Default for RubricRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProbe(RubricId, &'static str);

    impl LiveProbe for TestProbe {
        fn id(&self) -> RubricId {
            self.0
        }

        fn name(&self) -> &'static str {
            self.1
        }

        fn score_op(&self, op: &Operation, _state: &AgentState) -> Score {
            match op {
                Operation::Read => Score::Binary(true),
                _ => Score::None,
            }
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = RubricRegistry::new();
        reg.register(TestProbe(RubricId(1), "p1")).unwrap();
        reg.register(TestProbe(RubricId(2), "p2")).unwrap();
        assert_eq!(reg.len(), 2);
        assert!(reg.get(RubricId(1)).is_some());
        assert!(reg.get(RubricId(42)).is_none());
    }

    #[test]
    fn duplicate_id_fails() {
        let mut reg = RubricRegistry::new();
        reg.register(TestProbe(RubricId(1), "p1")).unwrap();
        assert_eq!(
            reg.register(TestProbe(RubricId(1), "p2")),
            Err(RegistryError::DuplicateId(RubricId(1)))
        );
    }

    #[test]
    fn reserved_id_fails() {
        let mut reg = RubricRegistry::new();
        assert_eq!(
            reg.register(TestProbe(NONE_RUBRIC_ID, "none")),
            Err(RegistryError::ReservedId(NONE_RUBRIC_ID))
        );
    }

    #[test]
    fn too_many_probes_fails() {
        let mut reg = RubricRegistry::new();
        for i in 0..RubricRegistry::MAX_LIVE_PROBES {
            let id = (i + 1) as u8; // avoid reserved0
            reg.register(TestProbe(RubricId(id), "p")).unwrap();
        }
        let res = reg.register(TestProbe(RubricId(100), "overflow"));
        assert!(matches!(res, Err(RegistryError::TooManyProbes { .. })));
    }
}
