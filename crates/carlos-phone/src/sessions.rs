//! Give the actual phone header and session picker typed, compact facts.

use carlos_proto::session::{Availability, Backend, Phase, SessionInfo};

/// The three native backends offered by the new-session picker.
pub type PhoneBackend = Backend;

#[uniffi::remote(Enum)]
enum PhoneBackend {
    Codex,
    Claude,
    Grok,
}

/// Approval takes precedence over an active turn in the phone's activity line.
pub type PhonePhase = Phase;

#[uniffi::remote(Enum)]
enum PhonePhase {
    Idle,
    Running,
    Approval,
}

/// A saved session may exist without a running native process.
pub type PhoneAvailability = Availability;

#[uniffi::remote(Enum)]
enum PhoneAvailability {
    Dormant,
    Restoring,
    Starting,
    Ready,
    Closed,
    Unavailable,
}

/// Activity facts never imply that an optimistic local action succeeded.
#[derive(uniffi::Record)]
pub struct PhoneActivity {
    pub phase: PhonePhase,
    pub availability: PhoneAvailability,
    pub ralph: bool,
    pub started_at: Option<i64>,
}

/// A backend-reported count; absent counts remain unknown.
#[derive(uniffi::Record)]
pub struct PhoneContext {
    pub used: u64,
    pub maximum: u64,
}

/// Absolute quota resets let Swift update its clock without a network request.
#[derive(uniffi::Record)]
pub struct PhoneQuota {
    pub remaining_percent: f64,
    pub resets_at: Option<i64>,
}

/// The phone shortens these header values for its available width.
#[derive(uniffi::Record)]
pub struct PhoneUsage {
    pub model: Option<String>,
    pub context: Option<PhoneContext>,
    pub quota: Option<PhoneQuota>,
}

/// One row in the session picker or the selected session's header.
#[derive(uniffi::Record)]
pub struct PhoneSession {
    pub id: String,
    pub name: Option<String>,
    pub backend: PhoneBackend,
    pub host: String,
    pub cwd: Option<String>,
    pub activity: PhoneActivity,
    pub usage: PhoneUsage,
}

impl From<&SessionInfo> for PhoneSession {
    fn from(info: &SessionInfo) -> Self {
        Self {
            id: info.id.to_string(),
            name: info.name.clone(),
            backend: info.backend,
            host: info.host.clone(),
            cwd: info.cwd.clone(),
            activity: PhoneActivity {
                phase: info.phase,
                availability: info.availability,
                ralph: info.ralph,
                started_at: info.turn_started_at,
            },
            usage: PhoneUsage {
                model: info.model.clone(),
                context: info.context.as_ref().map(|context| PhoneContext {
                    used: context.used,
                    maximum: context.max,
                }),
                quota: info.quota.as_ref().map(|quota| PhoneQuota {
                    remaining_percent: quota.remaining_pct,
                    resets_at: quota.resets_at,
                }),
            },
        }
    }
}
