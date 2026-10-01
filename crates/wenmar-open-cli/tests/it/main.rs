//! One test binary for the whole crate: each binary links SQLite again, and
//! disk space on the build machines is limited.

mod commands;
mod common;
mod data;
mod mcp;
mod output;
mod process;
mod remote;
mod server;
mod setup;
