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
    #[fatal(false)]
    Zero,

    #[error("X={0}")]
    #[fatal(forward)]
    A(#[source] X),

    #[error(transparent)]
    #[fatal(forward)]
    B(Y),

    #[error("X={0}")]
    #[fatal(forward)]
    Aaaaa(#[source] X),

    #[error(transparent)]
    #[fatal(forward)]
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
