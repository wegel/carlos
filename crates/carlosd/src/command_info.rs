//! Describe each wire command's handler and admission allowances in one exhaustive match.

use carlos_proto::commands::Command;

use crate::retry::Class;

/// Select the existing typed handler after the peer has authenticated.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handler {
    Hello,
    Devices,
    Prompts,
    Startup,
    Create,
    Stop,
    Reopen,
    Mutation,
    Watch,
    Operation,
    Read,
}

/// Reserve an independent count/byte allowance while preserving the common incoming FIFO.
#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum IngressClass {
    Ordinary,
    Control,
    Read,
    Lifecycle,
    Operator,
}

/// Keep handler choice, incoming allowance and optional retained-result allowance together.
#[derive(Clone, Copy)]
pub(crate) struct CommandInfo {
    pub handler: Handler,
    pub ingress: IngressClass,
    pub retry: Option<Class>,
}

const HELLO: CommandInfo = CommandInfo::read(Handler::Hello);
const DEVICES_READ: CommandInfo = CommandInfo {
    ingress: IngressClass::Operator,
    ..CommandInfo::read(Handler::Devices)
};
const DEVICES_WRITE: CommandInfo = CommandInfo::mutation(Handler::Devices, Class::Operator);
const PROMPTS: CommandInfo = CommandInfo::read(Handler::Prompts);
const STARTUP_READ: CommandInfo = CommandInfo::read(Handler::Startup);
const ARCHIVE: CommandInfo = CommandInfo::mutation(Handler::Startup, Class::Ordinary);
/// Preserve the open command's class through its asynchronous native-target check.
pub(crate) const OPEN: CommandInfo = CommandInfo::mutation(Handler::Startup, Class::Ordinary);
const CREATE: CommandInfo = CommandInfo::mutation(Handler::Create, Class::Ordinary);
const STOP: CommandInfo = CommandInfo::mutation(Handler::Stop, Class::Lifecycle);
const REOPEN: CommandInfo = CommandInfo::mutation(Handler::Reopen, Class::Ordinary);
const MUTATE: CommandInfo = CommandInfo::mutation(Handler::Mutation, Class::Ordinary);
const CONTROL: CommandInfo = CommandInfo::mutation(Handler::Mutation, Class::Control);
const WATCH: CommandInfo = CommandInfo::read(Handler::Watch);
const OPERATION: CommandInfo = CommandInfo::read(Handler::Operation);
const READ: CommandInfo = CommandInfo::read(Handler::Read);

/// Require every added command to choose its handler and allowances explicitly.
pub(crate) fn describe(command: &Command) -> CommandInfo {
    match command {
        Command::Hello(_) => HELLO,
        Command::DeviceAdd(_) | Command::DeviceRevoke(_) => DEVICES_WRITE,
        Command::DeviceList {} => DEVICES_READ,
        Command::PromptSelect { .. } | Command::PromptText { .. } => PROMPTS,
        Command::SavedArchive { .. } => ARCHIVE,
        Command::Open { .. } => OPEN,
        Command::SavedOpen { .. } | Command::SavedPage { .. } => STARTUP_READ,
        Command::SavedClose { .. } => STARTUP_READ,
        Command::Create { .. } => CREATE,
        Command::Stop(_) => STOP,
        Command::Reopen(_) => REOPEN,
        Command::Send { .. } | Command::Rename { .. } => MUTATE,
        Command::Rewind { .. } | Command::SetRalph { .. } => MUTATE,
        Command::ToggleRalph { .. } | Command::GoalSet { .. } => MUTATE,
        Command::GoalClear { .. } | Command::QueueChange { .. } => MUTATE,
        Command::ContextSet { .. } | Command::SettingsSet { .. } => MUTATE,
        Command::ToolLabelsSet { .. } => MUTATE,
        Command::TaskStop { .. } | Command::Approve { .. } => CONTROL,
        Command::Interrupt { .. } | Command::Reconnect { .. } => CONTROL,
        Command::Watch {} => WATCH,
        Command::Operation { .. } => OPERATION,
        Command::ContextRead { .. } | Command::SettingsRead { .. } => READ,
        Command::TaskActivityOpen { .. } | Command::TaskActivityRead { .. } => READ,
        Command::TaskActivityPage { .. } | Command::TaskActivityAdvance { .. } => READ,
        Command::TaskActivityClose { .. } | Command::TaskSubscribe { .. } => READ,
        Command::TaskUnsubscribe { .. } | Command::TaskOverview { .. } => READ,
        Command::TaskOverviewText { .. } | Command::TaskRun { .. } => READ,
        Command::TaskList { .. } | Command::TaskDetail { .. } => READ,
        Command::TaskText { .. } | Command::TaskWindowOpen { .. } => READ,
        Command::TaskWindowRead { .. } | Command::TaskWindowClose { .. } => READ,
        Command::QueueRead { .. } | Command::QueueText { .. } => READ,
        Command::GoalSubscribe { .. } | Command::GoalUnsubscribe { .. } => READ,
        Command::GoalRead { .. } | Command::Subscribe { .. } => READ,
        Command::Unsubscribe { .. } | Command::History { .. } => READ,
        Command::Output { .. } | Command::OutputSubscribe { .. } => READ,
        Command::OutputUnsubscribe { .. } | Command::Source { .. } => READ,
        Command::SourceSeek { .. } | Command::SourceSubscribe { .. } => READ,
        Command::SourceUnsubscribe { .. } => READ,
    }
}

impl CommandInfo {
    const fn read(handler: Handler) -> Self {
        Self {
            handler,
            ingress: IngressClass::Read,
            retry: None,
        }
    }

    const fn mutation(handler: Handler, retry: Class) -> Self {
        let ingress = match retry {
            Class::Ordinary => IngressClass::Ordinary,
            Class::Control => IngressClass::Control,
            Class::Lifecycle => IngressClass::Lifecycle,
            Class::Operator => IngressClass::Operator,
        };
        Self {
            handler,
            ingress,
            retry: Some(retry),
        }
    }
}
