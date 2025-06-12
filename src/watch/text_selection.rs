// Text selection functionality for watch mode activity logs
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};
use ratatui::layout::Rect;
use std::collections::VecDeque;

use crate::watch::events::WatchEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct TextSelection {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl TextSelection {
    pub fn new(start_line: usize, start_col: usize) -> Self {
        Self {
            start_line,
            start_col,
            end_line: start_line,
            end_col: start_col,
        }
    }

    pub fn update_end(&mut self, end_line: usize, end_col: usize) {
        self.end_line = end_line;
        self.end_col = end_col;
    }

    pub fn is_empty(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    pub fn contains_point(&self, line: usize, col: usize) -> bool {
        let (start_line, start_col, end_line, end_col) = self.normalized();
        
        if line < start_line || line > end_line {
            return false;
        }
        
        if line == start_line && line == end_line {
            col >= start_col && col <= end_col
        } else if line == start_line {
            col >= start_col
        } else if line == end_line {
            col <= end_col
        } else {
            true
        }
    }

    fn normalized(&self) -> (usize, usize, usize, usize) {
        if self.start_line < self.end_line || 
           (self.start_line == self.end_line && self.start_col <= self.end_col) {
            (self.start_line, self.start_col, self.end_line, self.end_col)
        } else {
            (self.end_line, self.end_col, self.start_line, self.start_col)
        }
    }
}

pub struct TextSelectionHandler {
    selection: Option<TextSelection>,
    is_selecting: bool,
    clipboard: Option<Clipboard>,
}

impl std::fmt::Debug for TextSelectionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSelectionHandler")
            .field("selection", &self.selection)
            .field("is_selecting", &self.is_selecting)
            .field("clipboard", &self.clipboard.is_some())
            .finish()
    }
}

impl TextSelectionHandler {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        Self {
            selection: None,
            is_selecting: false,
            clipboard,
        }
    }

    pub fn handle_mouse_event(
        &mut self,
        mouse_event: MouseEvent,
        area: Rect,
        _events: &VecDeque<WatchEvent>,
    ) -> Result<bool> {
        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.start_selection(mouse_event, area)?;
                Ok(true)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.update_selection(mouse_event, area)?;
                Ok(true)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.end_selection(mouse_event, area)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn start_selection(&mut self, mouse_event: MouseEvent, area: Rect) -> Result<()> {
        if self.is_mouse_in_area(mouse_event, area) {
            let (line, col) = self.mouse_to_text_position(mouse_event, area);
            self.selection = Some(TextSelection::new(line, col));
            self.is_selecting = true;
        }
        Ok(())
    }

    fn update_selection(&mut self, mouse_event: MouseEvent, area: Rect) -> Result<()> {
        if self.is_selecting && self.is_mouse_in_area(mouse_event, area) {
            let (line, col) = self.mouse_to_text_position(mouse_event, area);
            if let Some(ref mut selection) = self.selection {
                selection.update_end(line, col);
            }
        }
        Ok(())
    }

    fn end_selection(&mut self, mouse_event: MouseEvent, area: Rect) -> Result<()> {
        if self.is_selecting {
            if self.is_mouse_in_area(mouse_event, area) {
                let (line, col) = self.mouse_to_text_position(mouse_event, area);
                if let Some(ref mut selection) = self.selection {
                    selection.update_end(line, col);
                }
            }
            self.is_selecting = false;
        }
        Ok(())
    }

    fn is_mouse_in_area(&self, mouse_event: MouseEvent, area: Rect) -> bool {
        mouse_event.column >= area.x && 
        mouse_event.column < area.x + area.width &&
        mouse_event.row >= area.y && 
        mouse_event.row < area.y + area.height
    }

    fn mouse_to_text_position(&self, mouse_event: MouseEvent, area: Rect) -> (usize, usize) {
        let line = (mouse_event.row.saturating_sub(area.y).saturating_sub(1)) as usize; // Account for border
        let col = (mouse_event.column.saturating_sub(area.x).saturating_sub(1)) as usize; // Account for border
        (line, col)
    }

    pub fn copy_selected_text(&mut self, events: &VecDeque<WatchEvent>) -> Result<String> {
        if let Some(ref selection) = self.selection {
            if !selection.is_empty() {
                let selected_text = self.extract_selected_text(events, selection)?;
                if let Some(ref mut clipboard) = self.clipboard {
                    clipboard.set_text(&selected_text)?;
                }
                return Ok(selected_text);
            }
        }
        Ok(String::new())
    }

    fn extract_selected_text(&self, events: &VecDeque<WatchEvent>, selection: &TextSelection) -> Result<String> {
        let (start_line, start_col, end_line, end_col) = selection.normalized();
        let mut selected_lines = Vec::new();

        let event_lines: Vec<String> = events
            .iter()
            .rev()
            .map(|event| format_event_to_text(event))
            .collect();

        for (i, line) in event_lines.iter().enumerate() {
            if i >= start_line && i <= end_line {
                if start_line == end_line {
                    // Single line selection
                    let start = start_col.min(line.len());
                    let end = end_col.min(line.len());
                    if start < end {
                        selected_lines.push(line[start..end].to_string());
                    }
                } else if i == start_line {
                    // First line of multi-line selection
                    let start = start_col.min(line.len());
                    selected_lines.push(line[start..].to_string());
                } else if i == end_line {
                    // Last line of multi-line selection
                    let end = end_col.min(line.len());
                    selected_lines.push(line[..end].to_string());
                } else {
                    // Middle lines
                    selected_lines.push(line.clone());
                }
            }
        }

        Ok(selected_lines.join("\n"))
    }

    pub fn get_selection(&self) -> Option<&TextSelection> {
        self.selection.as_ref()
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.is_selecting = false;
    }

    pub fn has_selection(&self) -> bool {
        self.selection.as_ref().map_or(false, |s| !s.is_empty())
    }
}

fn format_event_to_text(event: &WatchEvent) -> String {
    let timestamp = event.get_timestamp().format("%H:%M:%S");
    match event {
        WatchEvent::NewMessage { tokens, cost, model, project, .. } => {
            format!("[{}] New message: {} tokens, ${:.4} ({}), project: {}", 
                   timestamp, tokens, cost, model, project)
        }
        WatchEvent::CacheHit { saved_tokens, saved_cost, project, .. } => {
            format!("[{}] Cache hit: saved {} tokens, ${:.4}, project: {}", 
                   timestamp, saved_tokens, saved_cost, project)
        }
        WatchEvent::ExpensiveConversation { cost, threshold, project, .. } => {
            format!("[{}] Expensive conversation: ${:.4} > ${:.4}, project: {}", 
                   timestamp, cost, threshold, project)
        }
        WatchEvent::ModelSwitch { from, to, project, .. } => {
            format!("[{}] Model switch: {} → {}, project: {}", 
                   timestamp, from, to, project)
        }
        WatchEvent::SessionStart { project, .. } => {
            format!("[{}] Session started: {}", timestamp, project)
        }
        WatchEvent::SessionEnd { project, duration, total_cost, .. } => {
            format!("[{}] Session ended: {}, duration: {}s, cost: ${:.4}", 
                   timestamp, project, duration.as_secs(), total_cost)
        }
        WatchEvent::ProjectActivity { project, message_count, cost, .. } => {
            format!("[{}] Project activity: {}, {} messages, ${:.4}", 
                   timestamp, project, message_count, cost)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::time::Duration;

    fn create_test_event(i: usize) -> WatchEvent {
        WatchEvent::NewMessage {
            tokens: 100 + i as u32,
            cost: 0.1 + i as f64 * 0.01,
            model: format!("test-model-{}", i),
            project: format!("test-project-{}", i),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_text_selection_creation() {
        let selection = TextSelection::new(1, 5);
        assert_eq!(selection.start_line, 1);
        assert_eq!(selection.start_col, 5);
        assert_eq!(selection.end_line, 1);
        assert_eq!(selection.end_col, 5);
        assert!(selection.is_empty());
    }

    #[test]
    fn test_text_selection_update() {
        let mut selection = TextSelection::new(1, 5);
        selection.update_end(2, 10);
        assert_eq!(selection.end_line, 2);
        assert_eq!(selection.end_col, 10);
        assert!(!selection.is_empty());
    }

    #[test]
    fn test_text_selection_contains_point() {
        let mut selection = TextSelection::new(1, 5);
        selection.update_end(3, 8);

        // Test points within selection
        assert!(selection.contains_point(1, 6));  // Start line, after start col
        assert!(selection.contains_point(2, 5));  // Middle line
        assert!(selection.contains_point(3, 6));  // End line, before end col

        // Test points outside selection
        assert!(!selection.contains_point(0, 10)); // Before start line
        assert!(!selection.contains_point(4, 5));  // After end line
        assert!(!selection.contains_point(1, 3));  // Start line, before start col
        assert!(!selection.contains_point(3, 10)); // End line, after end col
    }

    #[test]
    fn test_text_selection_normalized() {
        // Test forward selection
        let mut selection = TextSelection::new(1, 5);
        selection.update_end(3, 8);
        let (start_line, start_col, end_line, end_col) = selection.normalized();
        assert_eq!((start_line, start_col, end_line, end_col), (1, 5, 3, 8));

        // Test backward selection
        let mut selection = TextSelection::new(3, 8);
        selection.update_end(1, 5);
        let (start_line, start_col, end_line, end_col) = selection.normalized();
        assert_eq!((start_line, start_col, end_line, end_col), (1, 5, 3, 8));
    }

    #[test]
    fn test_selection_handler_creation() {
        let handler = TextSelectionHandler::new();
        assert!(!handler.has_selection());
        assert!(handler.get_selection().is_none());
    }

    #[test]
    fn test_mouse_in_area() {
        let handler = TextSelectionHandler::new();
        let area = Rect::new(10, 5, 20, 10);
        
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        
        assert!(handler.is_mouse_in_area(mouse_event, area));
        
        let mouse_event_outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        
        assert!(!handler.is_mouse_in_area(mouse_event_outside, area));
    }

    #[test]
    fn test_mouse_to_text_position() {
        let handler = TextSelectionHandler::new();
        let area = Rect::new(10, 5, 20, 10);
        
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        
        let (line, col) = handler.mouse_to_text_position(mouse_event, area);
        assert_eq!(line, 2); // row 8 - area.y 5 - border 1 = 2
        assert_eq!(col, 4);  // column 15 - area.x 10 - border 1 = 4
    }

    #[test]
    fn test_format_event_to_text() {
        let event = WatchEvent::NewMessage {
            tokens: 150,
            cost: 0.25,
            model: "test-model".to_string(),
            project: "test-project".to_string(),
            timestamp: Utc::now(),
        };
        
        let text = format_event_to_text(&event);
        assert!(text.contains("New message"));
        assert!(text.contains("150 tokens"));
        assert!(text.contains("$0.2500"));
        assert!(text.contains("test-model"));
        assert!(text.contains("test-project"));
    }

    #[test]
    fn test_extract_selected_text_single_line() {
        let handler = TextSelectionHandler::new();
        let mut events = VecDeque::new();
        events.push_back(create_test_event(0));
        events.push_back(create_test_event(1));
        
        let selection = TextSelection {
            start_line: 0,
            start_col: 10,
            end_line: 0,
            end_col: 20,
        };
        
        let result = handler.extract_selected_text(&events, &selection);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(!text.is_empty());
    }

    #[test]
    fn test_extract_selected_text_multi_line() {
        let handler = TextSelectionHandler::new();
        let mut events = VecDeque::new();
        events.push_back(create_test_event(0));
        events.push_back(create_test_event(1));
        events.push_back(create_test_event(2));
        
        let selection = TextSelection {
            start_line: 0,
            start_col: 10,
            end_line: 2,
            end_col: 15,
        };
        
        let result = handler.extract_selected_text(&events, &selection);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains('\n')); // Should have newlines for multi-line selection
    }
}