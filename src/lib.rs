#![deny(clippy::dbg_macro)]

//! Declarative annotations for `fatal` or `jfyi` error variants.
//!
//! Expand `#[derive(Split)]` annotations on error type definitions into two
//! additional error type definitions that can be converted back to the
//! original, or the original can be split into one of the two generated error
//! types.
//! Determination of fatality can also be forwarded to an inner error that
//! implements the `Fatality` trait.
//!
//! Stands on the shoulders of `thiserror`.

pub use error_fatality_proc_macro::{Fatality, Split};

/// Determine the fatality of an error.
pub trait Fatality: std::error::Error + std::fmt::Debug {
    /// Returns `true` if the error variant is _fatal_
    /// or `false` if it is more of a informational error.
    fn is_fatal(&self) -> bool;
}

/// Allows to split an error into two types - a fatal
/// and a informational enum error type, that can be further consumed.
pub trait Split: std::error::Error + std::fmt::Debug {
    type Fatal: std::error::Error;
    type Jfyi: std::error::Error;

    /// Split the error into it's fatal and non-fatal variants.
    ///
    /// `Ok(jfyi)` contains a enum representing all non-fatal variants, `Err(fatal)`
    /// contains all fatal variants.
    ///
    /// Attention: If the type is splitable, it must _not_ use any `forward`ed
    /// finality evaluations,
    /// or it must be splitable up the point where no more `forward`
    /// annotations were used.
    fn split(self) -> std::result::Result<Self::Jfyi, Self::Fatal>;
}

/// Converts a flat, yet `splitable` error into a nested `Result<Result<_,Jfyi>, Fatal>`
/// error type.
pub trait Nested<T, E: Split>
where
    Self: Sized,
{
    /// Convert into a nested error rather than a flat one, commonly for direct handling.
    fn into_nested(
        self,
    ) -> std::result::Result<
        std::result::Result<T, <E as Split>::Jfyi>,
        <E as Split>::Fatal,
    >;
}

impl<T, E: Split> Nested<T, E> for std::result::Result<T, E> {
    fn into_nested(
        self,
    ) -> std::result::Result<
        std::result::Result<T, <E as Split>::Jfyi>,
        <E as Split>::Fatal,
    > {
        match self {
            Ok(t) => Ok(Ok(t)),
            Err(e) => match e.split() {
                Ok(jfyi) => Ok(Err(jfyi)),
                Err(fatal) => Err(fatal),
            },
        }
    }
}

#[cfg(test)]
mod test;
