mod pit;

pub fn init() {
    pit::init();
}

/// Represents a periodic tick generator.
trait TickSource {
    /// Get an estimation of the ticks emitted per second.
    fn approx_ticks_per_second(&self) -> u64;
    // Set a function to be called for each tick.
    fn set_tick_handler(&mut self, fn(u64));
    // Get the current number of elapsed ticks.
    fn get_ticks(&self) -> u64;
}
