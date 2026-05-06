
[![crates.io](https://img.shields.io/crates/v/fatality.svg)](https://crates.io/crates/fatality)
[![CI](https://ci.fff.rs/api/v1/teams/main/pipelines/fatality/jobs/master-validate/badge)](https://ci.fff.rs/teams/main/pipelines/fatality/jobs/master-validate)
![commits-since](https://img.shields.io/github/commits-since/drahnr/fatality/latest.svg)
[![rust 1.51.0+ badge](https://img.shields.io/badge/rust-1.51.0+-93450a.svg)](https://blog.rust-lang.org/2021/03/25/Rust-1.51.0.html)

# fatality

A generative approach to creating _fatal_ and _non-fatal_ errors.

The generated source utilizes `thiserror::Error` derived attributes heavily,
and any unknown annotations will be passed to that.

## Motivation

For large scale mono-repos, with subsystems it eventually becomes very tedious to `match`
against nested error variants defined with `thiserror`. Using `anyhow` or `eyre` - while it being an application - also comes with an unmanagable amount of pain for medium-large scale code bases.

`fatality` is a solution to this, by extending `thiserror::Error` with annotations to declare certain variants as `fatal`, or `forward` the fatality extraction to an inner error type.

Read on!

## Usage

`#[derive(Fatality)]` currently provides a `trait Fatality` with a single `fn is_fatal(&self) -> bool` by default.

Annotations with `forward` require the _inner_ error type to also implement `trait Fatality`.

Annotating with `#[derive(Split)]`, allows to split the type into two sub-types, a `Jfyi*` and a `Fatal*` one via `fn split(self) -> Result<Self::Jfyi, Self::Fatal>`.

The derive macro implements them, and can defer calls, based on `thiserror` annotations, specifically
`#[source]` and `#[transparent]` on `enum` variants and their members.


```rust
use fatality::Fatality;
use thiserror::Error;

#[derive(Debug, Error, Fatality)]
enum OhMy {
    #[error("An apple a day")]
    #[fatal(false)]
    Itsgonnabefine,

    /// Forwards the `is_fatal` to the `InnerError`, which has to implement `trait Fatality` as well.
    #[error("Dropped dead")]
    #[fatal(forward)]
    ReallyReallyBad(#[source] InnerError),

    /// Also works on `#[error(transparent)]
    #[error(transparent)]
    #[fatal(forward)]
    Translucent(InnerError),


    /// Will always return `is_fatal` as `true`,
    /// irrespective of `#[error(transparent)]` or
    /// `#[source]` annotations.
    #[error("So dead")]
    #[fatal(true)]
    SoDead(#[source] InnerError),
}
```

```rust
use fatality::{Fatality, Split};
use thiserror::Error;

#[derive(Debug, Error, Fatality, Split)]
enum Yikes {
    #[error("An apple a day")]
    #[fatal(false)]
    Orange,

    #[error("So dead")]
    #[fatal(true)]
    Dead,
}

fn foo() -> Result<[u8;32], Yikes> {
    Err(Yikes::Dead)
}

fn i_call_foo() -> Result<(), FatalYikes> {
    // availble via a convenience trait `Nested` that is implemented
    // for any `Result` whose error type implements `Split`.
    let x: Result<[u8;32], Jfyi> = foo().into_nested()?;
}

fn i_call_foo_too() -> Result<(), FatalYikes> {
    if let Err(jfyi_and_fatal_ones) = foo() {
        // bail if bad, otherwise just log it
        log::warn!("Jfyi: {:?}", jfyi_and_fatal_ones.split()?);
    }
}
```

## Derive options
`#[derive(Fatality)]` and `#[derive(Split)]` support a number of options via
attributes.

### `#[derive(Fatality)]`

The `#[fatal(_)]` attribute is mandatory on all enum variants and structs.
This specifies the fatality of an error.
There are three possible values: `true` for fatal errors, `false` for non-fatal
errors, and `forward` if fatality should be determined by the error source field.

### `#[derive(Split)]`

By default, `#[derive(Split)]` will generate a `#[derive(::std::fmt::Debug, ::thiserror::Error)] attribute on each of the generated split error types.
`#[derive(Split)]` also copies all other attributes.

```rust
use fatality::{Fatality, Split};
use thiserror::Error;

#[derive(Debug, Error, Fatality, Split)]
#[error(transparent)]
#[fatal(forward)]
#[repr(transparent)]
struct Outer(#[from] Inner);
```

generates

```rust
#[derive(::std::fmt::Debug, ::thiserror::Error)]
#[error(transparent)]
#[repr(transparent)]
struct FatalOuter(#[from] <Inner as crate::Split>::Fatal);

#[automatically_derived]
impl ::std::convert::From<FatalOuter> for Outer {
    fn from(fatal: FatalOuter) -> Self {
        Self { 0: Inner::from(fatal.0), }
    }
}

#[derive(::std::fmt::Debug, ::thiserror::Error)]
#[error(transparent)]
#[repr(transparent)]
struct JfyiOuter(#[from] <Inner as crate::Split>::Jfyi);

#[automatically_derived]
impl ::std::convert::From<JfyiOuter> for Outer {
    fn from(jfyi: JfyiOuter) -> Self {
        Self { 0: Inner::from(jfyi.0), }
    }
}

#[automatically_derived]
impl crate::Split for Outer {
    type Fatal = FatalOuter;
    type Jfyi = JfyiOuter;
    fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
        match crate :: Split :: split (self . 0) {
            Err(fatal) => Err(FatalOuter { 0: fatal, }),
            Ok(jfyi) => Ok(JfyiOuter { 0: jfyi, }),
        }
    }
}
```

It is possible to manually specify the attributes that will be applied to
each of the generated error variants, as well as the identifiers used:

```rust
use fatality::{Fatality, Split};
use thiserror::Error;

#[derive(Debug, Error, Fatality, Split)]
#[error(transparent)]
#[fatal(forward)]
#[repr(transparent)]
#[split(
    // These options apply only to the fatal variant
    fatal(ident = "CustomFatalIdent"),
    // These options apply only to the non-fatal variant
    jfyi(
        attrs(
            derive(Debug, Default, Error),
            error("non-fatal error: {0}"),
            repr(transparent),
        ),
        ident = "CustomJfyiIdent",
    ),
    // These options apply to both variants, unless the same option is
    // provided for a specific variant, in which case they apply to the
    // other variant.
    // Because the `attrs` option is specified for the non-fatal variant,
    // these attributes will apply to the fatal variant.
    attrs(
        derive(Debug, Error),
        error(transparent),
        repr(transparent),
    ),
)]
struct Outer(#[from] Inner);
```

generates

```rust
#[derive(Debug, Error)]
#[error(transparent)]
#[repr(transparent)]
struct CustomFatalIdent(#[from] <Inner as crate::Split>::Fatal);

#[automatically_derived]
impl ::std::convert::From<CustomFatalIdent> for Outer {
    fn from(fatal: CustomFatalIdent) -> Self {
        Self { 0: Inner::from(fatal.0), }
    }
}

#[derive(Debug, Default, Error)]
#[error("non-fatal error: {0}")]
#[repr(transparent)]
struct CustomJfyiIdent(#[from] <Inner as crate::Split>::Jfyi);

#[automatically_derived]
impl ::std::convert::From<CustomJfyiIdent> for Outer {
    fn from(jfyi: CustomJfyiIdent) -> Self {
        Self { 0: Inner::from(jfyi.0), }
    }
}

#[automatically_derived]
impl crate::Split for Outer {
    type Fatal = CustomFatalIdent;
    type Jfyi = CustomJfyiIdent;
    fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
        match crate :: Split :: split (self . 0) {
            Err(fatal) => Err(CustomFatalIdent { 0: fatal, }),
            Ok(jfyi) => Ok(CustomJfyiIdent { 0: jfyi, }),
        }
    }
}
```

