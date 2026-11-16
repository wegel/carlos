//! Reject a stale native-goal read while preserving unrelated newer transcript events.

use carlos_proto::events::{Event, Notification};
use carlos_proto::goals::{GoalFacts, GoalKnowledge, GoalSnapshot};
use carlos_proto::ids::Sequence;

use super::client::{attached, greet, notify, reply};
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

#[test]
fn client_goal_reads_use_one_fact_copy_and_reject_future_or_superseded_cursors() {
    let (mut client, connection) = attached();
    let changes = client.merge_goal(read(10, "a")).expect("current read");
    assert_eq!(changes.sessions.len(), 1);
    assert!(changes.sessions[0].changes.goal);
    assert_eq!(
        client.session(&session()).expect("facts").goal(),
        Some(&facts("a"))
    );
    assert!(matches!(
        client.merge_goal(read(11, "b")),
        Err(ClientError::StaleGoal)
    ));
    assert_eq!(
        client.session(&session()).expect("unchanged").goal(),
        Some(&facts("a"))
    );
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::GoalChanged {
                goal: Some(facts("b")),
            },
        )),
    );
    assert!(matches!(
        client.merge_goal(read(10, "a")),
        Err(ClientError::StaleGoal)
    ));
    assert_eq!(
        client.session(&session()).expect("current").goal(),
        Some(&facts("b"))
    );
    assert!(client
        .merge_goal(read(11, "b"))
        .expect("equal read")
        .sessions
        .is_empty());
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            13,
            Event::GoalChanged {
                goal: Some(facts("d")),
            },
        )),
    );
    assert!(matches!(
        client.merge_goal(read(11, "c")),
        Err(ClientError::StaleGoal)
    ));
}

#[test]
fn a_new_daemon_adopts_lower_sequence_goal_facts_instead_of_retaining_an_old_pending_write() {
    let (mut client, connection) = attached();
    let mut pending = read(10, "a");
    pending.goal.pending = true;
    let _ = client.merge_goal(pending).expect("old pending fact");
    client.disconnect(connection);
    let (_, welcome) = greet(&mut client, false, "1");
    let mut reset = snapshot();
    reset.seq = Sequence::new(1);
    reset.goal = Some(facts("b"));
    let _ = reply(
        &mut client,
        &welcome.outgoing[0],
        serde_json::json!({"kind":"snapshot", "snapshot":reset}),
    );
    let replica = client.session(&session()).expect("new daemon facts");
    assert_eq!(replica.sequence().get(), 1);
    assert_eq!(replica.goal(), Some(&facts("b")));
    assert!(!replica.goal().expect("new goal state").pending);
}
