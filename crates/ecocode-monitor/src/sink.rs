//! Where records go. The CLI's exporters implement [`Sink`].

use std::error::Error;

use crate::record::Record;

pub trait Sink {
    /// Called once per sample, from the sampling loop. Must not block for long:
    /// slow destinations (network) should queue and send in the background.
    fn write(&mut self, record: &Record) -> Result<(), Box<dyn Error>>;
}

impl Sink for Vec<Record> {
    fn write(&mut self, record: &Record) -> Result<(), Box<dyn Error>> {
        self.push(record.clone());
        Ok(())
    }
}
