use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, MouseEventKind};

pub(crate) fn read_batch() -> Result<Vec<Event>> {
    let mut events = Vec::new();
    if !event::poll(Duration::from_millis(8)).context("failed to poll terminal events")? {
        return Ok(events);
    }
    let start = Instant::now();
    while events.len() < 64 {
        let event = event::read().context("failed to read terminal event")?;
        let passive = matches!(
            event,
            Event::Mouse(mouse) if matches!(mouse.kind,
                MouseEventKind::Moved | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight)
        );
        events.push(event);
        if !passive
            || start.elapsed() >= Duration::from_millis(8)
            || !event::poll(Duration::ZERO).context("failed to poll terminal events")?
        {
            break;
        }
    }
    Ok(events)
}
