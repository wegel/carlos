//! Apply a native goal read only when no later numbered goal fact replaced it.

use carlos_proto::goals::{GoalFacts, GoalSnapshot};
use carlos_proto::ids::Sequence;

use crate::{ChangeSet, ClientError, Replica};

impl Replica {
    /// Keep unrelated newer transcript deltas while refusing late goal reads or replacement races.
    pub fn merge_goal(&mut self, snapshot: GoalSnapshot) -> Result<ChangeSet, ClientError> {
        if snapshot.session != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        if self.stale || snapshot.seq < self.goal_seq || snapshot.seq > self.seq {
            return Err(ClientError::StaleGoal);
        }
        let mut changed = ChangeSet::default();
        self.replace_goal(Some(snapshot.goal), snapshot.seq, &mut changed);
        Ok(changed)
    }

    pub(crate) fn replace_goal(
        &mut self,
        goal: Option<GoalFacts>,
        seq: Sequence,
        changed: &mut ChangeSet,
    ) {
        changed.goal = self.goal != goal;
        self.goal = goal;
        self.goal_seq = seq;
    }
}
