//! Reject a stale native-goal read while preserving unrelated newer transcript events.

use carlos_proto::events::Event;
use carlos_proto::goals::{GoalFacts, GoalKnowledge, GoalSnapshot};
use carlos_proto::ids::Sequence;

use super::{entry_id, event, session, snapshot};
use crate::{ClientError, Replica};

fn facts(revision: &str) -> GoalFacts {
    GoalFacts {
        revision: revision.repeat(64).parse().expect("goal revision"),
        knowledge: GoalKnowledge::Known,
        goal: None,
        reading: false,
        pending: false,
        direct_input: None,
        read_error: None,
        notice: String::new(),
    }
}

fn read(seq: u64, revision: &str) -> GoalSnapshot {
    GoalSnapshot {
        session: session(),
        seq: Sequence::new(seq),
        goal: facts(revision),
    }
}

#[test]
fn numbered_goal_changes_retire_only_late_goal_reads_and_snapshots_restore_the_same_facts() {
    let mut initial = snapshot();
    initial.goal = Some(facts("a"));
    let mut replica = Replica::new(initial).expect("snapshot");
    replica
        .apply(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: " new".into(),
            },
        ))
        .expect("unrelated text");
    assert!(!replica.merge_goal(read(10, "a")).expect("safe read").goal);
    assert!(
        replica
            .apply(event(
                12,
                Event::GoalChanged {
                    goal: Some(facts("b"))
                }
            ))
            .expect("goal change")
            .goal
    );
    assert_eq!(
        replica
            .merge_goal(read(11, "a"))
            .expect_err("late old goal"),
        ClientError::StaleGoal
    );
    assert_eq!(replica.goal(), Some(&facts("b")));
    assert_eq!(replica.entry(&entry_id(10)).expect("row").text, "hello new");
    let mut reset = snapshot();
    reset.seq = Sequence::new(14);
    reset.goal = Some(facts("c"));
    replica
        .apply(event(
            14,
            Event::Reset {
                snapshot: Box::new(reset),
            },
        ))
        .expect("reset");
    assert_eq!(replica.goal(), Some(&facts("c")));
    assert_eq!(
        replica
            .merge_goal(read(12, "b"))
            .expect_err("before replacement"),
        ClientError::StaleGoal
    );
}
