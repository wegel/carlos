//! Bind card-local questions without exporting their JSON values or native callback identities.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, Weak};

use carlos_proto::commands::{ApprovalAnswer, Command, FieldAnswer, FieldAnswers};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::session::{ApprovalInput, InputField};

use crate::client::State;
use crate::{PhoneClient, PhoneError, PhoneSubmission};

/// Numeric keyboards never convert private text to a lossy native floating-point value.
#[derive(uniffi::Enum)]
pub enum PhoneFieldKind {
    Text,
    Integer,
    Number,
    Choice,
}

/// Fields and browser consent need different native controls; unsupported input stays explicit.
#[derive(uniffi::Enum)]
pub enum PhoneApprovalInput {
    Fields { questions: Arc<PhoneQuestions> },
    Form { questions: Arc<PhoneQuestions> },
    Browser { url: String },
    Unsupported { reason: String },
}

/// A choice's position belongs only to the captured question; its original value stays in Rust.
#[derive(uniffi::Record)]
pub struct PhoneQuestionOption {
    pub label: String,
    pub description: String,
}

/// Native controls need labels and editing rules, not an independently interpreted schema.
#[derive(uniffi::Record)]
pub struct PhoneQuestionField {
    pub key: String,
    pub title: String,
    pub description: String,
    pub secret: bool,
    pub multiple: bool,
    pub other: bool,
    pub notes_only: bool,
    pub required: bool,
    pub kind: PhoneFieldKind,
    pub options: Vec<PhoneQuestionOption>,
}

/// Selected positions and exact private text never contain a backend request id.
#[derive(uniffi::Record)]
pub struct PhoneQuestionAnswer {
    pub key: String,
    pub selected: Vec<u32>,
    pub text: Option<String>,
}

/// Capture exactly one host, card and field definition before the user starts editing.
#[derive(uniffi::Object)]
pub struct PhoneQuestions {
    owner: Weak<Mutex<State>>,
    session: SessionId,
    approval: EntryId,
    fields: Vec<InputField>,
    form: Option<serde_json::Value>,
}

#[uniffi::export]
impl PhoneQuestions {
    /// Read native field labels without exposing the original JSON choice values.
    pub fn fields(&self) -> Vec<PhoneQuestionField> {
        self.fields.iter().map(PhoneQuestionField::from).collect()
    }

    /// Forms distinguish an omitted optional property from an explicitly empty value.
    pub fn is_form(&self) -> bool {
        self.form.is_some()
    }

    /// Keep edits through reconnects, but never silently transfer them to different questions.
    pub fn same_questions(&self, other: Arc<Self>) -> bool {
        Weak::ptr_eq(&self.owner, &other.owner)
            && self.session == other.session
            && self.approval == other.approval
            && self.fields == other.fields
            && self.form == other.form
    }
}

impl PhoneQuestions {
    pub(crate) fn new(
        owner: &Arc<Mutex<State>>,
        session: SessionId,
        approval: EntryId,
        fields: Vec<InputField>,
    ) -> Arc<Self> {
        Arc::new(Self {
            owner: Arc::downgrade(owner),
            session,
            approval,
            fields,
            form: None,
        })
    }

    pub(crate) fn form(
        owner: &Arc<Mutex<State>>,
        session: SessionId,
        approval: EntryId,
        schema: serde_json::Value,
    ) -> Result<Arc<Self>, PhoneError> {
        if schema.to_string().len() > 65536 {
            return Err(invalid("Form schema exceeds 64 KiB"));
        }
        let fields =
            carlos_proto::forms::fields(&schema).map_err(|error| invalid(&error.to_string()))?;
        if fields.iter().any(|field| {
            !matches!(
                field.schema["type"].as_str(),
                Some("string" | "number" | "integer" | "boolean" | "array")
            ) || matches!(field.schema["type"].as_str(), Some("boolean" | "array"))
                && field.options.is_empty()
        }) {
            return Err(invalid(
                "This form contains a field that cannot be edited on this device",
            ));
        }
        Ok(Arc::new(Self {
            owner: Arc::downgrade(owner),
            session,
            approval,
            fields,
            form: Some(schema),
        }))
    }

    fn encode(&self, answers: Vec<PhoneQuestionAnswer>) -> Result<serde_json::Value, PhoneError> {
        let answers = checked_fields(&self.fields, answers)?;
        if self.form.is_some() {
            return crate::form_answers::encode(answers);
        }
        let fields = answers
            .into_iter()
            .map(|(field, answer)| encode_field(field, answer))
            .collect::<Result<_, _>>()?;
        let value = serde_json::to_value(FieldAnswers { fields })
            .map_err(|_| invalid("Cannot encode this answer"))?;
        bounded(value)
    }

    fn matches(&self, input: &Option<ApprovalInput>) -> bool {
        match (&self.form, input) {
            (None, Some(ApprovalInput::Fields { fields })) => fields == &self.fields,
            (Some(expected), Some(ApprovalInput::Form { schema })) => schema == expected,
            _ => false,
        }
    }
}

#[uniffi::export]
impl PhoneClient {
    /// Recheck the original card and map only its offered positions before admitting a private reply.
    pub fn answer_questions(
        &self,
        questions: Arc<PhoneQuestions>,
        answers: Vec<PhoneQuestionAnswer>,
    ) -> Result<PhoneSubmission, PhoneError> {
        if !Weak::ptr_eq(&questions.owner, &Arc::downgrade(&self.state)) {
            return Err(invalid("These questions belong to a different host"));
        }
        let mut state = self.lock()?;
        let current = state
            .client
            .session(&questions.session)
            .and_then(|session| {
                session
                    .approvals()
                    .iter()
                    .find(|card| card.id == questions.approval)
            });
        let current = current.filter(|card| card.active && !card.submitted);
        if !state
            .client
            .current_session_has_snapshot(&questions.session)
            || !current.is_some_and(|card| questions.matches(&card.input))
        {
            return Err(PhoneError::StaleRead {
                message: "These questions changed or closed; your answers are retained".into(),
            });
        }
        let value = questions.encode(answers)?;
        state.submit(|operation| Command::Approve {
            session: questions.session.clone(),
            approval: questions.approval.clone(),
            operation,
            answer: ApprovalAnswer::Input { value },
        })
    }
}

impl From<&InputField> for PhoneQuestionField {
    fn from(field: &InputField) -> Self {
        Self {
            key: field.key.clone(),
            title: field.title.clone(),
            description: field.description.clone(),
            secret: field.secret,
            multiple: field.multiple,
            other: field.other || field.options.is_empty(),
            notes_only: field.notes_only,
            required: field.required,
            kind: if !field.options.is_empty() {
                PhoneFieldKind::Choice
            } else {
                match field.schema["type"].as_str() {
                    Some("integer") => PhoneFieldKind::Integer,
                    Some("number") => PhoneFieldKind::Number,
                    _ => PhoneFieldKind::Text,
                }
            },
            options: field
                .options
                .iter()
                .map(|option| PhoneQuestionOption {
                    label: option.label.clone(),
                    description: option.description.clone(),
                })
                .collect(),
        }
    }
}

fn encode_field(
    field: &InputField,
    answer: PhoneQuestionAnswer,
) -> Result<FieldAnswer, PhoneError> {
    let values = selected_values(field, answer.selected)?;
    let text = answer.text.filter(|text| !text.is_empty());
    if text.is_some() && (!field.other || field.notes_only && values.is_empty()) {
        return Err(invalid("Choose an offered answer before adding a note"));
    }
    if field.required && values.is_empty() && text.is_none() {
        return Err(invalid("Answer every required question"));
    }
    Ok(FieldAnswer {
        key: answer.key,
        values,
        text,
    })
}

pub(crate) fn selected_values(
    field: &InputField,
    selected: Vec<u32>,
) -> Result<Vec<serde_json::Value>, PhoneError> {
    if !field.multiple && selected.len() > 1 {
        return Err(invalid("Choose only one answer for this question"));
    }
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for index in selected {
        if !seen.insert(index) {
            return Err(invalid("A choice is selected more than once"));
        }
        let option = field
            .options
            .get(index as usize)
            .ok_or_else(|| invalid("This choice is no longer offered"))?;
        values.push(option.value.clone());
    }
    Ok(values)
}

pub(crate) fn checked_fields(
    fields: &[InputField],
    answers: Vec<PhoneQuestionAnswer>,
) -> Result<Vec<(&InputField, PhoneQuestionAnswer)>, PhoneError> {
    if answers.len() > fields.len() {
        return Err(invalid("Answer contains too many fields"));
    }
    let mut seen = HashSet::new();
    let mut checked = Vec::new();
    for answer in answers {
        if answer.text.as_ref().is_some_and(|text| text.len() > 65536) {
            return Err(invalid("Keep the complete answer within 64 KiB"));
        }
        if !seen.insert(answer.key.clone()) {
            return Err(invalid("Answer repeats a field"));
        }
        let field = fields
            .iter()
            .find(|field| field.key == answer.key)
            .ok_or_else(|| invalid("Answer names a different field"))?;
        checked.push((field, answer));
    }
    if fields
        .iter()
        .any(|field| field.required && !seen.contains(&field.key))
    {
        return Err(invalid("Answer every required field"));
    }
    Ok(checked)
}

pub(crate) fn bounded(value: serde_json::Value) -> Result<serde_json::Value, PhoneError> {
    if value.to_string().len() > 65536 {
        return Err(invalid("Keep the complete answer within 64 KiB"));
    }
    Ok(value)
}

pub(crate) fn invalid(message: &str) -> PhoneError {
    PhoneError::InvalidInput {
        message: message.into(),
    }
}
