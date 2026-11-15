//! Run captured priority controls through their original guarded native writer.

use std::sync::atomic::Ordering;
use std::time::Duration;

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Approval;
use serde_json::{json, Value};

use super::approval_state::{ApprovalChoice, ApprovalClaim};
use super::control_work::{ControlCall, ControlFailure};
use crate::backend_commands::CommandHandle;

pub(super) fn execute(
    call: &ControlCall,
    client: &CommandHandle,
    timeout: Duration,
) -> Result<(Option<ApprovalChoice>, Value), ControlFailure> {
    match call {
        ControlCall::QuestionInterrupt { claim, params, .. } => {
            if !claim.live.load(Ordering::SeqCst) {
                return Err(ControlFailure::Retired);
            }
            client
                .call("turn/interrupt", params.clone(), timeout)
                .and_then(|line| crate::protocol_params::extract_result_object(&line))
                .map(|value| (None, value))
                .map_err(|error| ControlFailure::Native(error.to_string()))
        }
        ControlCall::NativeAnswer { claim, body } => {
            if !claim.live.load(Ordering::SeqCst) {
                return Err(ControlFailure::Retired);
            }
            client
                .respond(&claim.facts.request_id, body.clone())
                .map_err(|error| ControlFailure::Native(error.to_string()))?;
            Ok((Some(ApprovalChoice::Accept), json!(null)))
        }
        ControlCall::Interrupt { params, live, .. } => {
            if !live.load(Ordering::SeqCst) {
                return Err(ControlFailure::Retired);
            }
            client
                .call("turn/interrupt", params.clone(), timeout)
                .and_then(|line| crate::protocol_params::extract_result_object(&line))
                .map(|value| (None, value))
                .map_err(|error| ControlFailure::Native(error.to_string()))
        }
        ControlCall::Answer {
            claim,
            card,
            answer,
        } => answer_card(claim, card, answer, client),
    }
}

fn answer_card(
    claim: &ApprovalClaim,
    card: &Approval,
    answer: &ApprovalAnswer,
    client: &CommandHandle,
) -> Result<(Option<ApprovalChoice>, Value), ControlFailure> {
    if !claim.live.load(Ordering::SeqCst) {
        return Err(ControlFailure::Retired);
    }
    let (choice, body) = super::control_answers::convert(&claim.facts, card, answer)
        .map_err(|error| ControlFailure::Invalid(error.to_string()))?;
    if body.to_string().len() > 1024 * 1024 {
        return Err(ControlFailure::Invalid("answer exceeds one MiB".into()));
    }
    if !claim.live.load(Ordering::SeqCst) {
        return Err(ControlFailure::Retired);
    }
    client
        .respond(&claim.facts.request_id, body)
        .map_err(|error| {
            if matches!(
                error.downcast_ref::<crate::claude_backend::ControllerFailure>(),
                Some(crate::claude_backend::ControllerFailure::Rejected(_))
            ) {
                ControlFailure::Invalid(error.to_string())
            } else {
                ControlFailure::Native(error.to_string())
            }
        })?;
    Ok((Some(choice), json!(null)))
}
