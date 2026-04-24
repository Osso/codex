//! User-message display models and helpers for the chat widget.
//!
//! The app-server preserves user input as structured chunks, while chat history
//! renders a single prompt row. This module owns that display projection and
//! the small compare key used to suppress duplicate rows for pending steers.

use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::UserInput;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;

use super::ChatWidget;
use super::append_text_with_rebased_elements;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct UserMessageDisplay {
    pub(super) message: String,
    pub(super) remote_image_urls: Vec<String>,
    pub(super) local_images: Vec<PathBuf>,
    pub(super) text_elements: Vec<TextElement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingSteerCompareKey {
    pub(super) message: String,
    pub(super) image_count: usize,
}

fn normalized_pending_steer_message(message: &str) -> String {
    let mut remaining = message;

    while let Some(stripped) = strip_known_prepended_context_block(remaining) {
        remaining = stripped;
    }

    remaining.to_string()
}

fn strip_known_prepended_context_block(message: &str) -> Option<&str> {
    strip_graph_context_block(message).or_else(|| strip_plan_hook_block(message))
}

fn strip_graph_context_block(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("Graph context:\n")?;
    let crlf = "\r\n\r\n";
    if let Some(separator_idx) = rest.find(crlf) {
        return Some(&rest[separator_idx + crlf.len()..]);
    }

    let lf = "\n\n";
    let separator_idx = rest.find(lf)?;
    Some(&rest[separator_idx + lf.len()..])
}

fn strip_plan_hook_block(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("```sh\n")?;
    let fence_end = rest.find("\n```")?;
    let command = &rest[..fence_end];
    let command_basename = shlex::split(command)
        .and_then(|parts| parts.into_iter().next())
        .and_then(|program| {
            Path::new(&program)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        })?;
    if command_basename != "claude-plan-hook" {
        return None;
    }

    let after_fence = &rest[fence_end + "\n```".len()..];
    after_fence
        .strip_prefix("\r\n\r\n")
        .or_else(|| after_fence.strip_prefix("\n\n"))
}

impl ChatWidget {
    pub(super) fn user_message_display_from_parts(
        message: String,
        text_elements: Vec<TextElement>,
        local_images: Vec<PathBuf>,
        remote_image_urls: Vec<String>,
    ) -> UserMessageDisplay {
        let (message, prompt_request_offset) =
            crate::ide_context::extract_prompt_request_with_offset(&message);
        let prompt_request_end = prompt_request_offset + message.len();
        // Prompt context uses the same delimiter and stripping behavior as the desktop app and IDE
        // extension. The raw user message goes to the agent, but every surface renders only the
        // request after that delimiter, so keep elements inside the visible request and shift their
        // byte ranges to match.
        let text_elements = text_elements
            .into_iter()
            .filter_map(|element| {
                let range = element.byte_range;
                if range.start < prompt_request_offset || range.end > prompt_request_end {
                    return None;
                }

                Some(element.map_range(|range| ByteRange {
                    start: range.start - prompt_request_offset,
                    end: range.end - prompt_request_offset,
                }))
            })
            .collect();

        UserMessageDisplay {
            message: message.to_string(),
            remote_image_urls,
            local_images,
            text_elements,
        }
    }

    /// Build the compare key for a submitted pending steer without invoking the
    /// expensive request-serialization path. Pending steers only need to match the
    /// committed app-server `UserMessage` item emitted after input drains, which
    /// preserves flattened text and total image count.
    pub(super) fn pending_steer_compare_key_from_items(
        items: &[UserInput],
    ) -> PendingSteerCompareKey {
        let mut message = String::new();
        let mut image_count = 0;

        for item in items {
            match item {
                UserInput::Text { text, .. } => message.push_str(text),
                UserInput::Image { .. } | UserInput::LocalImage { .. } => image_count += 1,
                UserInput::Skill { .. } | UserInput::Mention { .. } => {}
            }
        }

        PendingSteerCompareKey {
            message: normalized_pending_steer_message(&message),
            image_count,
        }
    }

    pub(super) fn user_message_display_from_inputs(items: &[UserInput]) -> UserMessageDisplay {
        let mut message = String::new();
        let mut remote_image_urls = Vec::new();
        let mut local_images = Vec::new();
        let mut text_elements = Vec::new();

        for item in items {
            match item {
                UserInput::Text {
                    text,
                    text_elements: current_text_elements,
                } => append_text_with_rebased_elements(
                    &mut message,
                    &mut text_elements,
                    text,
                    current_text_elements.iter().map(|element| {
                        let range = element.byte_range.clone();
                        TextElement::new(
                            range.clone().into(),
                            element
                                .placeholder()
                                .or_else(|| text.get(range.start..range.end))
                                .map(str::to_string),
                        )
                    }),
                ),
                UserInput::Image { url } => remote_image_urls.push(url.clone()),
                UserInput::LocalImage { path } => local_images.push(path.clone()),
                UserInput::Skill { .. } | UserInput::Mention { .. } => {}
            }
        }

        Self::user_message_display_from_parts(
            message,
            text_elements,
            local_images,
            remote_image_urls,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ChatWidget;
    use super::PendingSteerCompareKey;
    use codex_app_server_protocol::UserInput;
    use pretty_assertions::assert_eq;

    #[test]
    fn pending_steer_compare_key_strips_prepended_plan_hook_context_blocks() {
        let items = vec![UserInput::Text {
            text: "```sh\n~/Projects/claude/claude-plan-hook --fast\n```\r\n\r\nGraph context:\n- deploy bot maintained_by user\r\n\r\nrun tests".to_string(),
            text_elements: Vec::new(),
        }];

        assert_eq!(
            ChatWidget::pending_steer_compare_key_from_items(&items),
            PendingSteerCompareKey {
                message: "run tests".to_string(),
                image_count: 0,
            }
        );
    }
}
