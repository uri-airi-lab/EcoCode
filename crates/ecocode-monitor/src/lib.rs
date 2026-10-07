//! The EcoCode measurement engine, identical on every OS.
//!
//! - [`sampler`]: THE measurement loop: reads sensors, attributes energy,
//!   builds one [`record::Record`] per tick
//! - [`attribution`]: splits whole-domain energy by the process tree's share
//! - [`markers`]: phase / step / epoch / loss marker messages from the Python SDK
//! - [`record`]: one sample = energy + markers + timestamp
//! - [`sink`]: the output trait exporters implement

pub mod attribution;
pub mod markers;
pub mod record;
pub mod sampler;
pub mod sink;

pub use markers::MarkerState;
pub use record::Record;
pub use sampler::{Control, LoopEnd, Sampler, run_loop};
pub use sink::Sink;
