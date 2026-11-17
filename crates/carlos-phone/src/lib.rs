//! Expose the shared checked client to Swift while the phone owns its socket and UI.

mod approval_inputs;
mod approvals;
mod bodies;
mod body_views;
mod changes;
mod client;
mod controls;
mod document;
mod entries;
mod error;
mod file_lines;
mod form_answers;
mod operations;
mod records;
mod session_calls;
mod sessions;
mod syntax;
mod transcript;

pub use approval_inputs::{
    PhoneApprovalInput, PhoneFieldKind, PhoneQuestionAnswer, PhoneQuestionField,
    PhoneQuestionOption, PhoneQuestions,
};
pub use approvals::{PhoneApproval, PhoneApprovalCaption, PhoneDecision};
pub use bodies::PhoneBody;
pub use body_views::{PhoneOutputKind, PhoneOutputPiece, PhoneOutputWindow, PhoneSourceWindow};
pub use changes::{PhoneSessionChange, PhoneWindowChange};
pub use client::PhoneClient;
pub use document::{
    PhoneDocument, PhoneMarkdownBlock, PhoneMarkdownColumn, PhoneMarkdownKind, PhoneMarkdownSpan,
    PhoneMarkdownStyle,
};
pub use entries::{
    PhoneDiff, PhoneEntry, PhoneEntryFormat, PhoneEntryKind, PhoneLabelSource, PhoneText,
    PhoneTool, PhoneToolKind, PhoneToolStatus,
};
pub use error::PhoneError;
pub use file_lines::{file_lines, PhoneFileLine};
pub use operations::{PhoneOperation, PhoneOperationState, PhoneSubmission};
pub use records::{PhoneBatch, PhoneFailure, PhoneFrame};
pub use sessions::{
    PhoneActivity, PhoneAvailability, PhoneBackend, PhoneContext, PhonePhase, PhoneQuota,
    PhoneSession, PhoneUsage,
};
pub use syntax::{highlight_code, PhoneSyntaxRole, PhoneSyntaxSpan};
pub use transcript::PhoneTranscript;

uniffi::setup_scaffolding!();

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod session_call_tests;

#[cfg(test)]
mod control_tests;

#[cfg(test)]
mod question_tests;

#[cfg(test)]
mod body_tests;

#[cfg(test)]
mod history_tests;

#[cfg(test)]
mod syntax_tests;
