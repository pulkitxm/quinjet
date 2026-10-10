use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, MouseEventKind};

#[derive(Debug, Default)]
pub(crate) struct Reader {
    pending: Option<Event>,
}

impl Reader {
    pub(crate) fn read_batch(&mut self) -> Result<Vec<Event>> {
        if let Some(event) = self.pending.take() {
            return Ok(vec![event]);
        }
        let mut events = Vec::new();
        if !event::poll(Duration::from_millis(8)).context("failed to poll terminal events")? {
            return Ok(events);
        }
        let start = Instant::now();
        while events.len() < 64 {
            let event = event::read().context("failed to read terminal event")?;
            if !self.push_event(&mut events, event)
                || start.elapsed() >= Duration::from_millis(8)
                || !event::poll(Duration::ZERO).context("failed to poll terminal events")?
            {
                break;
            }
        }
        Ok(events)
    }

    fn push_event(&mut self, events: &mut Vec<Event>, event: Event) -> bool {
        let passive = matches!(
            event,
            Event::Mouse(mouse) if matches!(mouse.kind,
                MouseEventKind::Moved | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight)
        );
        if !passive && !events.is_empty() {
            self.pending = Some(event);
        } else {
            events.push(event);
        }
        passive
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};

    use super::*;

    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 50,
            row: 10,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn clicks_keys_and_resize_wait_for_the_scroll_frame_to_render() {
        for next in [
            mouse(MouseEventKind::Down(MouseButton::Left)),
            Event::Key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE)),
            Event::Resize(120, 32),
        ] {
            let mut reader = Reader::default();
            let mut events = Vec::new();
            let wheel = mouse(MouseEventKind::ScrollDown);
            assert!(reader.push_event(&mut events, wheel.clone()));
            assert!(!reader.push_event(&mut events, next.clone()));
            assert_eq!(events, vec![wheel]);
            assert_eq!(reader.read_batch().unwrap(), vec![next]);
            assert!(reader.pending.is_none());
        }
    }

    #[test]
    fn passive_mouse_events_are_kept_in_arrival_order() {
        let mut reader = Reader::default();
        let mut events = Vec::new();
        let input = [
            mouse(MouseEventKind::ScrollDown),
            mouse(MouseEventKind::Moved),
            mouse(MouseEventKind::ScrollUp),
        ];
        for event in &input {
            assert!(reader.push_event(&mut events, event.clone()));
        }
        assert_eq!(events, input);
        assert!(reader.pending.is_none());
    }
}
