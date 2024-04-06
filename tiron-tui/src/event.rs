use tiron_common::action::ActionMessage;
use uuid::Uuid;

pub enum AppEvent {
    UserInput(UserInputEvent),
    Run(RunEvent),
    Action {
        run: Uuid,
        host: Uuid,
        msg: ActionMessage,
    },
}

pub enum UserInputEvent {
    ScrollUp,
    ScrollDown,
    PrevRun,
    NextRun,
    PrevHost,
    NextHost,
    Quit,
}

pub enum RunEvent {
    RunStarted { id: Uuid },
    RunCompleted { id: Uuid, success: bool },
}
