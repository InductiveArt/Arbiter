use crate::execution::event::Event;

// Hears each event as it happens.
pub trait Observer {
    fn event(&mut self, event: Event<'_>);
}

// Hears everything and says nothing.
pub struct Quiet;

impl Observer for Quiet {
    fn event(&mut self, _event: Event<'_>) {}
}
