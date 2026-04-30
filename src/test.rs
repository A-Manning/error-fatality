use thiserror::Error;

use crate::Fatality;

#[derive(Debug, Error)]
#[error("X")]
struct X;

impl Fatality for X {
    fn is_fatal(&self) -> bool {
        false
    }
}

#[derive(Debug, Error)]
#[error("Y")]
struct Y;

impl Fatality for Y {
    fn is_fatal(&self) -> bool {
        true
    }
}

#[derive(Debug, Error, Fatality)]
enum Acc {
    #[error("0")]
    Zero,

    #[error("X={0}")]
    A(#[source] X),

    #[fatal]
    #[error(transparent)]
    B(Y),

    #[fatal(forward)]
    #[error("X={0}")]
    Aaaaa(#[source] X),

    #[fatal(forward)]
    #[error(transparent)]
    Bbbbbb(Y),
}

#[test]
fn all_in_one() {
    assert!(!Fatality::is_fatal(&Acc::A(X)));
    assert!(Fatality::is_fatal(&Acc::B(Y)));
    assert!(!Fatality::is_fatal(&Acc::Aaaaa(X)));
    assert!(Fatality::is_fatal(&Acc::Bbbbbb(Y)));
    assert!(!Fatality::is_fatal(&Acc::Zero));
}
