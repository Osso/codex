use crate::slash_command::SlashCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlashCommandAction {
    SubmitPrompt(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlashCommandHandler {
    pub(crate) action: SlashCommandAction,
}

pub(crate) fn handler_for(command: SlashCommand) -> Option<SlashCommandHandler> {
    let action = match command {
        SlashCommand::Init => {
            SlashCommandAction::SubmitPrompt(include_str!("../prompt_for_init_command.md"))
        }
        SlashCommand::SpecValidation => SlashCommandAction::SubmitPrompt(include_str!(
            "../prompt_for_spec_validation_command.md"
        )),
        _ => return None,
    };

    Some(SlashCommandHandler { action })
}
