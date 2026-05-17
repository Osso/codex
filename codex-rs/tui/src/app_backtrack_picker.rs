use std::any::TypeId;
use std::sync::Arc;

use crate::app_backtrack::BacktrackSelection;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::SelectionItem;
use crate::history_cell::HistoryCell;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;

pub(crate) fn backtrack_picker_items(cells: &[Arc<dyn HistoryCell>]) -> Vec<SelectionItem> {
    let mut items = Vec::new();
    let user_positions: Vec<usize> = user_positions_iter(cells).collect();

    for (nth_user_message, cell_idx) in user_positions.into_iter().enumerate().rev() {
        let Some(user_cell) = cells
            .get(cell_idx)
            .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
        else {
            continue;
        };
        let selection = BacktrackSelection {
            nth_user_message,
            prefill: user_cell.message.clone(),
            text_elements: user_cell.text_elements.clone(),
            local_image_paths: user_cell.local_image_paths.clone(),
            remote_image_urls: user_cell.remote_image_urls.clone(),
        };
        items.push(SelectionItem {
            name: one_line_prompt_label(&user_cell.message),
            actions: vec![Box::new(move |tx: &AppEventSender| {
                tx.send(AppEvent::ApplyBacktrackSelection(selection.clone()));
            })],
            dismiss_on_select: true,
            search_value: Some(user_cell.message.clone()),
            ..Default::default()
        });
    }

    items
}

fn one_line_prompt_label(message: &str) -> String {
    let label = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if label.is_empty() {
        "(empty prompt)".to_string()
    } else {
        label
    }
}

fn user_positions_iter(cells: &[Arc<dyn HistoryCell>]) -> impl Iterator<Item = usize> + '_ {
    let session_start_type = TypeId::of::<SessionInfoCell>();
    let user_type = TypeId::of::<UserHistoryCell>();
    let type_of = |cell: &Arc<dyn HistoryCell>| cell.as_any().type_id();

    let start = cells
        .iter()
        .rposition(|cell| type_of(cell) == session_start_type)
        .map_or(0, |idx| idx + 1);

    cells
        .iter()
        .enumerate()
        .skip(start)
        .filter_map(move |(idx, cell)| (type_of(cell) == user_type).then_some(idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::AgentMessageCell;
    use ratatui::prelude::Line;

    fn user_cell(message: &str) -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            message: message.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>
    }

    #[test]
    fn backtrack_picker_items_show_one_prompt_per_line_newest_first() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            user_cell("first prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("answer")],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>,
            user_cell("second prompt\nwith details"),
        ];

        let items = backtrack_picker_items(&cells);

        assert_eq!(
            items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second prompt with details", "first prompt"]
        );
    }
}
