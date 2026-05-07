//! Tests for `#[derive(Split)]`

use proc_macro2::TokenStream;
use quote::quote;

fn run_test(input: TokenStream, expected: TokenStream) {
    let output = crate::derive_split2(input);
    let output = output.to_string();
    assert_eq!(output, expected.to_string());
}

#[test]
fn simple() {
    run_test(
        quote! {
            enum Kaboom {
                #[error("Eh?")]
                #[fatal(false)]
                Eh,

                #[error("Explosion")]
                #[fatal(true)]
                Explosion,
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            enum FatalKaboom {
                #[error("Explosion")]
                Explosion
            }

            #[automatically_derived]
            impl ::std::convert::From<FatalKaboom> for Kaboom {
                fn from(fatal: FatalKaboom) -> Self {
                    match fatal {
                        FatalKaboom::Explosion => Self::Explosion,
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            enum JfyiKaboom {
                #[error("Eh?")]
                Eh
            }

            #[automatically_derived]
            impl ::std::convert::From<JfyiKaboom> for Kaboom {
                fn from(jfyi: JfyiKaboom) -> Self {
                    match jfyi {
                        JfyiKaboom::Eh => Self::Eh,
                    }
                }
            }

            #[automatically_derived]
            impl crate::Split for Kaboom {
                type Fatal = FatalKaboom;
                type Jfyi = JfyiKaboom;

                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        // Fatal
                        Self::Explosion => Err(FatalKaboom::Explosion),
                        // JFYI
                        Self::Eh => Ok(JfyiKaboom::Eh),
                    }
                }
            }
        },
    );
}

#[test]
fn strukt_cannot_split() {
    run_test(
        quote! {
            #[error("Cancelled")]
            pub struct X;
        },
        quote! {
            ::core::compile_error! { "missing `#[fatal(_)]` attribute for struct" }
        },
    );
    run_test(
        quote! {
            #[error("Cancelled")]
            #[fatal(forward)]
            pub struct X;
        },
        quote! {
            ::core::compile_error! { "cannot forward to a unit item variant" }
        },
    );
}

#[test]
fn regression() {
    run_test(
        quote! {
            pub enum X {
                #[error("Cancelled")]
                #[fatal(true)]
                Inner(Foo),
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum FatalX {
                #[error("Cancelled")]
                Inner (Foo)
            }

            #[automatically_derived]
            impl :: std :: convert :: From < FatalX > for X {
                fn from (fatal : FatalX) -> Self {
                    match fatal {
                        FatalX :: Inner(arg_0) => Self :: Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            #[automatically_derived]
            impl :: std :: convert :: From < JfyiX > for X {
                fn from (jfyi : JfyiX) -> Self {
                    match jfyi {
                    }
                }
            }

            #[automatically_derived]
            impl crate :: Split for X {
                type Fatal = FatalX ;
                type Jfyi = JfyiX ;
                fn split (self) -> :: std :: result :: Result < Self :: Jfyi , Self :: Fatal > {
                    match self {
                        Self::Inner(arg_0) => Err (FatalX :: Inner(arg_0)) ,
                    }
                }
            }
        },
    );
}

#[test]
fn generics_and_bounds_implicit() {
    run_test(
        quote! {
            pub enum Outer<'a, Inner> where
                Inner: std::error::Error
            {
                #[error(transparent)]
                #[fatal(true)]
                Fatal(&'a Inner),
                #[error(transparent)]
                #[fatal(false)]
                Jfyi(&'a Inner),
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum FatalOuter<'a, Inner> where Inner: std::error::Error {
                #[error(transparent)]
                Fatal(&'a Inner)
            }

            #[automatically_derived]
            impl<'a, Inner> ::std::convert::From< FatalOuter<'a, Inner> > for
                Outer<'a, Inner> where Inner: std::error::Error
            {
                fn from(fatal: FatalOuter<'a, Inner>) -> Self {
                    match fatal {
                        FatalOuter::Fatal(arg_0) => Self::Fatal(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiOuter<'a, Inner> where Inner: std::error::Error {
                #[error(transparent)]
                Jfyi(&'a Inner)
            }

            #[automatically_derived]
            impl<'a, Inner> ::std::convert::From< JfyiOuter<'a, Inner> > for
                Outer<'a, Inner> where Inner: std::error::Error
            {
                fn from(jfyi: JfyiOuter<'a, Inner>) -> Self {
                    match jfyi {
                        JfyiOuter::Jfyi(arg_0) => Self::Jfyi(arg_0),
                    }
                }
            }

            #[automatically_derived]
            impl<'a, Inner> crate::Split for Outer<'a, Inner> where
                Inner: std::error::Error
            {
                type Fatal = FatalOuter<'a, Inner>;
                type Jfyi = JfyiOuter<'a, Inner>;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        Self::Fatal(arg_0) => Err(FatalOuter::Fatal(arg_0)),
                        Self::Jfyi(arg_0) => Ok(JfyiOuter::Jfyi(arg_0)),
                    }
                }
            }
        },
    );
    run_test(
        quote! {
            #[error(transparent)]
            #[fatal(forward)]
            struct Outer<'a, T>(Inner<'a, T>) where T: std::error::Error;
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            #[error(transparent)]
            struct FatalOuter<'a, T>(<Inner<'a, T> as crate::Split>::Fatal) where T: std::error::Error;

            #[automatically_derived]
            impl<'a, T> ::std::convert::From< FatalOuter<'a, T> > for
                Outer<'a, T> where T: std::error::Error
            {
                fn from(fatal: FatalOuter<'a, T>) -> Self {
                    Self { 0: <Inner<'a, T> as ::std::convert::From<_>>::from(fatal.0), }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            #[error(transparent)]
            struct JfyiOuter<'a, T>(<Inner<'a, T> as crate::Split>::Jfyi) where T: std::error::Error;

            #[automatically_derived]
            impl<'a, T> ::std::convert::From< JfyiOuter<'a, T> > for
                Outer<'a, T> where T: std::error::Error
            {
                fn from(jfyi: JfyiOuter<'a, T>) -> Self {
                    Self { 0: <Inner<'a, T> as ::std::convert::From<_>>::from(jfyi.0), }
                }
            }

            #[automatically_derived]
            impl<'a, T> crate::Split for Outer<'a, T> where
                T: std::error::Error
            {
                type Fatal = FatalOuter<'a, T>;
                type Jfyi = JfyiOuter<'a, T>;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match crate :: Split :: split (self . 0) {
                        Err(fatal) => Err(FatalOuter { 0: fatal, }),
                        Ok(jfyi) => Ok(JfyiOuter { 0: jfyi, }),
                    }
                }
            }
        },
    )
}

#[test]
fn generics_and_bounds_explicit() {
    run_test(
        quote! {
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
            pub enum E<'a, Fatal, Jfyi> where
                Fatal: std::error::Error,
                Jfyi: std::error::Error,
            {
                #[error(transparent)]
                #[fatal(true)]
                Fatal(&'a Fatal),
                #[error(transparent)]
                #[fatal(false)]
                Jfyi(&'a Jfyi),
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum FatalE<'a, Fatal> where Fatal: std::error::Error {
                #[error(transparent)]
                Fatal(&'a Fatal)
            }

            #[automatically_derived]
            impl<'a, Fatal, Jfyi> ::std::convert::From<
                FatalE<'a, Fatal>
            > for E<'a, Fatal, Jfyi> where
                Fatal: std::error::Error,
                Jfyi: std::error::Error,
            {
                fn from(fatal: FatalE<'a, Fatal>) -> Self {
                    match fatal {
                        FatalE::Fatal(arg_0) => Self::Fatal(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiE<'a, Jfyi> where Jfyi: std::error::Error {
                #[error(transparent)]
                Jfyi(&'a Jfyi)
            }

            #[automatically_derived]
            impl<'a, Fatal, Jfyi> ::std::convert::From<
                JfyiE<'a, Jfyi>
            > for E<'a, Fatal, Jfyi> where
                Fatal: std::error::Error,
                Jfyi: std::error::Error,
            {
                fn from(jfyi: JfyiE<'a, Jfyi>) -> Self {
                    match jfyi {
                        JfyiE::Jfyi(arg_0) => Self::Jfyi(arg_0),
                    }
                }
            }

            #[automatically_derived]
            impl<'a, Fatal, Jfyi> crate::Split for E<'a, Fatal, Jfyi> where
                Fatal: std::error::Error,
                Jfyi: std::error::Error,
            {
                type Fatal = FatalE<'a, Fatal>;
                type Jfyi = JfyiE<'a, Jfyi>;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        Self::Fatal(arg_0) => Err(FatalE::Fatal(arg_0)),
                        Self::Jfyi(arg_0) => Ok(JfyiE::Jfyi(arg_0)),
                    }
                }
            }
        },
    )
}

#[test]
fn no_attrs() {
    let output = quote! {
        pub enum FatalX {
            #[error("Cancelled")]
            Inner(Foo)
        }

        #[automatically_derived]
        impl ::std::convert::From<FatalX> for X {
            fn from(fatal: FatalX) -> Self {
                match fatal {
                    FatalX::Inner(arg_0) => Self::Inner(arg_0),
                }
            }
        }

        pub enum JfyiX { }

        #[automatically_derived]
        impl ::std::convert::From<JfyiX> for X {
            fn from(jfyi: JfyiX) -> Self {
                match jfyi { }
            }
        }

        #[automatically_derived]
        impl crate::Split for X {
            type Fatal = FatalX;
            type Jfyi = JfyiX;
            fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                match self {
                    Self::Inner(arg_0) => Err(FatalX::Inner(arg_0)) ,
                }
            }
        }
    };
    run_test(
        quote! {
            #[split(attrs())]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output.clone(),
    );
    run_test(
        quote! {
            #[split(fatal(attrs()), attrs())]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output.clone(),
    );
    run_test(
        quote! {
            #[split(attrs(), fatal(attrs()))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output.clone(),
    );
    run_test(
        quote! {
            #[split(jfyi(attrs()), attrs())]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output.clone(),
    );
    run_test(
        quote! {
            #[split(attrs(), jfyi(attrs()))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output.clone(),
    );
    run_test(
        quote! {
            #[split(fatal(attrs()), jfyi(attrs()))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        output,
    );
}

#[test]
fn no_fatal_attrs() {
    run_test(
        quote! {
            #[split(fatal(attrs()))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        quote! {
            pub enum FatalX {
                #[error("Cancelled")]
                Inner(Foo)
            }

            #[automatically_derived]
            impl ::std::convert::From<FatalX> for X {
                fn from(fatal: FatalX) -> Self {
                    match fatal {
                        FatalX::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            #[automatically_derived]
            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

            #[automatically_derived]
            impl crate::Split for X {
                type Fatal = FatalX;
                type Jfyi = JfyiX;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        Self::Inner(arg_0) => Err(FatalX::Inner(arg_0)) ,
                    }
                }
            }
        },
    );
}

#[test]
fn no_jfyi_attrs() {
    run_test(
        quote! {
            #[split(jfyi(attrs()))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum FatalX {
                #[error("Cancelled")]
                Inner(Foo)
            }

            #[automatically_derived]
            impl ::std::convert::From<FatalX> for X {
                fn from(fatal: FatalX) -> Self {
                    match fatal {
                        FatalX::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            pub enum JfyiX { }

            #[automatically_derived]
            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

            #[automatically_derived]
            impl crate::Split for X {
                type Fatal = FatalX;
                type Jfyi = JfyiX;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        Self::Inner(arg_0) => Err(FatalX::Inner(arg_0)) ,
                    }
                }
            }
        },
    );
}

#[test]
fn rename_fatal() {
    run_test(
        quote! {
            #[split(fatal(ident = "RenamedFatalXRenamed"))]
            pub enum X {
                #[fatal(true)]
                #[error("Cancelled")]
                Inner(Foo),
            }
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum RenamedFatalXRenamed {
                #[error("Cancelled")]
                Inner(Foo)
            }

            #[automatically_derived]
            impl ::std::convert::From<RenamedFatalXRenamed> for X {
                fn from(fatal: RenamedFatalXRenamed) -> Self {
                    match fatal {
                        RenamedFatalXRenamed::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            #[automatically_derived]
            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

            #[automatically_derived]
            impl crate::Split for X {
                type Fatal = RenamedFatalXRenamed;
                type Jfyi = JfyiX;
                fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                    match self {
                        Self::Inner(arg_0) => Err(RenamedFatalXRenamed::Inner(arg_0)) ,
                    }
                }
            }
        },
    );
}

#[test]
fn cloned_attributes() {
    run_test(
        quote! {
            #[error(transparent)]
            #[fatal(forward)]
            #[repr(transparent)]
            struct Outer(#[from] Inner);
        },
        quote! {
            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            #[error(transparent)]
            #[repr(transparent)]
            struct FatalOuter(#[from] <Inner as crate::Split>::Fatal);

            #[automatically_derived]
            impl ::std::convert::From<FatalOuter> for Outer {
                fn from(fatal: FatalOuter) -> Self {
                    Self { 0: <Inner as ::std::convert::From<_>>::from(fatal.0), }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            #[error(transparent)]
            #[repr(transparent)]
            struct JfyiOuter(#[from] <Inner as crate::Split>::Jfyi);

            #[automatically_derived]
            impl ::std::convert::From<JfyiOuter> for Outer {
                fn from(jfyi: JfyiOuter) -> Self {
                    Self { 0: <Inner as ::std::convert::From<_>>::from(jfyi.0), }
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
        },
    );
}

#[test]
fn all_attr_options() {
    run_test(
        quote! {
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
        },
        quote! {
            #[derive(Debug, Error)]
            #[error(transparent)]
            #[repr(transparent)]
            struct CustomFatalIdent(#[from] <Inner as crate::Split>::Fatal);

            #[automatically_derived]
            impl ::std::convert::From<CustomFatalIdent> for Outer {
                fn from(fatal: CustomFatalIdent) -> Self {
                    Self { 0: <Inner as ::std::convert::From<_>>::from(fatal.0), }
                }
            }

            #[derive(Debug, Default, Error)]
            #[error("non-fatal error: {0}")]
            #[repr(transparent)]
            struct CustomJfyiIdent(#[from] <Inner as crate::Split>::Jfyi);

            #[automatically_derived]
            impl ::std::convert::From<CustomJfyiIdent> for Outer {
                fn from(jfyi: CustomJfyiIdent) -> Self {
                    Self { 0: <Inner as ::std::convert::From<_>>::from(jfyi.0), }
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
        },
    );
}
