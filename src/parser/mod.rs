/// Parser for the main act XML file (`<ACT>` root).
mod act;
/// Parser for annex XML files (`<ANNEX>` root).
mod annex;

pub use act::parse_act;
pub use annex::parse_annex;
