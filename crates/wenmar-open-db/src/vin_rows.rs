//! The rows one VIN needs, read before decoding. The code is in
//! `wenmar-vehicles`, beside the `Source` it reads through, so that the
//! WebAssembly build of the decoder shares it.

pub use wenmar_vehicles::vin_rows::{VinRows, fetch};
