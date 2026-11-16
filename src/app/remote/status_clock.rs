//! Advance local footer animation and expire only the temporary notice that this clock owns.

use std::time::{Duration, Instant};

#[derive(Default)]
pub(in crate::app::remote) struct StatusClock {
    pub(in crate::app::remote) minute: Option<Instant>,
    animation: Option<u128>,
    notice: Option<(String, Instant)>,
}

impl StatusClock {
    pub(super) fn tick(&mut self, now: Instant, animation: u128, running: bool) -> bool {
        let animated = running && self.animation != Some(animation);
        self.animation = Some(animation);
        let minute = self.minute.is_none_or(|until| until <= now);
        if minute {
            self.minute = Some(now + Duration::from_secs(60));
        }
        animated || minute
    }

    pub(super) fn notice(&mut self, hint: &mut Option<String>, message: String, now: Instant) {
        self.notice = Some((message.clone(), now + Duration::from_secs(2)));
        *hint = Some(message);
    }

    pub(super) fn expire_notice(&mut self, hint: &mut Option<String>, now: Instant) -> bool {
        let Some((message, until)) = &self.notice else {
            return false;
        };
        if hint.as_ref() != Some(message) {
            self.notice = None;
            return false;
        }
        if now < *until {
            return false;
        }
        self.notice = None;
        *hint = None;
        true
    }
}
