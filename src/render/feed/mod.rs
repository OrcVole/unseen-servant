//! Feed emitters: a directory's [`crate::render::metadata::FeedEntry`]
//! list rendered as the two formats each surface needs — [`atom`] for the
//! web, [`gemsub`] for Gemini. Both consume the exact same entries, so
//! the two surfaces can never list different content or disagree on
//! dates, the property ADR 0004 exists to guarantee.

pub mod atom;
pub mod gemsub;
