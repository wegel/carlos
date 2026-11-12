//! Enumerates handler interrupts and reader lag, comparing complete ordered facts with main.

use super::lifecycle_schedule_support::{base, check, disruptions, Counts, Interrupt};

#[test]
fn bounded_handler_schedules_match_main_in_status_and_fact_order() {
    let mut counts = Counts::default();
    for variant in 0..6 {
        let facts = base(1, variant);
        enumerate_lag(&facts, &mut counts);
        enumerate_disruptions(&facts, &mut counts);
        enumerate_two_calls(&facts, &mut counts);
    }
    for first in 0..6 {
        for variant in [first * 7, first + ((first + 1) % 6) * 6] {
            enumerate_lag(&base(2, variant), &mut counts);
        }
    }
    eprintln!(
        "bounded schedules={}, depth={}",
        counts.schedules, counts.depth
    );
    assert_eq!(counts.schedules, 46_458);
    assert_eq!(counts.depth, 29);
}

// Full 1,221,882-schedule enumeration: cargo test -- --ignored.
#[test]
#[ignore = "full lifecycle schedule enumeration; run cargo test -- --ignored"]
fn generated_handler_schedules_match_main_in_status_and_complete_fact_order() {
    let mut counts = Counts::default();
    for prompts in 1..=3 {
        for variant in 0..6_usize.pow(prompts as u32) {
            let facts = base(prompts, variant);
            enumerate_lag(&facts, &mut counts);
        }
        for variant in 0..6 {
            // All prompts use this success/error/abort and idle/result variant.
            let repeated = (0..prompts).map(|p| variant * 6_usize.pow(p as u32)).sum();
            let facts = base(prompts, repeated);
            enumerate_disruptions(&facts, &mut counts);
            enumerate_two_calls(&facts, &mut counts);
        }
    }
    eprintln!(
        "differential schedules={}, maximum primitive depth={}",
        counts.schedules, counts.depth
    );
    assert_eq!(counts.schedules, 1_221_882);
    assert_eq!(counts.depth, 47);
}

fn enumerate_lag(facts: &[String], counts: &mut Counts) {
    check(facts, &[], false, counts);
    for handled in 0..=facts.len() {
        for read in handled..=facts.len() {
            for reply in read..=facts.len() {
                for accepted in [Some(true), Some(false), None] {
                    for forwarded in [false, true] {
                        check(
                            facts,
                            &[Interrupt {
                                handled,
                                read,
                                reply,
                                accepted,
                            }],
                            forwarded,
                            counts,
                        );
                    }
                }
            }
        }
    }
}

fn enumerate_disruptions(facts: &[String], counts: &mut Counts) {
    for extra in disruptions() {
        for position in 0..=facts.len() {
            let mut schedule = facts.to_vec();
            schedule.splice(position..position, extra.clone());
            for handled in 0..=schedule.len() {
                for reply in handled..=schedule.len() {
                    for accepted in [Some(true), Some(false), None] {
                        // Include both no lag and a reader already past the interrupt reply.
                        for read in [handled, reply] {
                            check(
                                &schedule,
                                &[Interrupt {
                                    handled,
                                    read,
                                    reply,
                                    accepted,
                                }],
                                true,
                                counts,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn enumerate_two_calls(facts: &[String], counts: &mut Counts) {
    for first in 0..=facts.len() {
        for second in first..=facts.len() {
            for accepted_first in [Some(true), Some(false)] {
                for accepted_second in [Some(true), Some(false), None] {
                    let calls = [
                        Interrupt {
                            handled: first,
                            read: facts.len(),
                            reply: facts.len(),
                            accepted: accepted_first,
                        },
                        Interrupt {
                            handled: second,
                            read: facts.len(),
                            reply: facts.len(),
                            accepted: accepted_second,
                        },
                    ];
                    check(facts, &calls, true, counts);
                }
            }
        }
    }
}
