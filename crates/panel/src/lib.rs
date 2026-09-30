//! The panel's engine: [`wire`] turns protojson into the domain of `panel_core`; [`store`]
//! is Postgres; [`seal`] encrypts what must not sit in the clear.

pub mod seal;
pub mod store;
pub mod wire;
