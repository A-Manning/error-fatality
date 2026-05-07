use error_fatality::{Fatality, Split};
use thiserror::Error;

#[derive(Debug, Error, Fatality, Split)]
#[split(
    fatal(
        bound(Fatal: std::error::Error),
        generics('a, Fatal),
    ),
    jfyi(
        bound(Jfyi: std::error::Error),
        generics('a, Jfyi),
    )
)]
enum Enum<'a, Fatal, Jfyi> where
    Fatal: std::error::Error,
    Jfyi: std::error::Error,
{
    #[error(transparent)]
    #[fatal(true)]
    F(&'a Fatal),
    #[error(transparent)]
    #[fatal(false)]
    J(&'a Jfyi),
}

#[derive(Debug, Error, Fatality, Split)]
#[error(transparent)]
#[fatal(forward)]
struct Struct<'a, Fatal, Jfyi>(Enum<'a, Fatal, Jfyi>) where
    Fatal: std::error::Error,
    Jfyi: std::error::Error;

fn main() { }
