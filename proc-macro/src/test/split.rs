//! Tests for `#[derive(Split)]`

use proc_macro2::TokenStream;
use quote::quote;

fn run_test(input: TokenStream, expected: TokenStream) {
    let output = crate::derive_split2(input);
    let output = output.to_string();
    println!(
        r##">>>>>>>>>>>>>>>>>>>
{}
>>>>>>>>>>>>>>>>>>>"##,
        output.as_str()
    );
    assert_eq!(output, expected.to_string(),);
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

            impl ::std::convert::From<JfyiKaboom> for Kaboom {
                fn from(jfyi: JfyiKaboom) -> Self {
                    match jfyi {
                        JfyiKaboom::Eh => Self::Eh,
                    }
                }
            }

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

            impl :: std :: convert :: From < FatalX > for X {
                fn from (fatal : FatalX) -> Self {
                    match fatal {
                        FatalX :: Inner(arg_0) => Self :: Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            impl :: std :: convert :: From < JfyiX > for X {
                fn from (jfyi : JfyiX) -> Self {
                    match jfyi {
                    }
                }
            }

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
fn no_attrs() {
    let output = quote! {
        pub enum FatalX {
            #[error("Cancelled")]
            Inner(Foo)
        }

        impl ::std::convert::From<FatalX> for X {
            fn from(fatal: FatalX) -> Self {
                match fatal {
                    FatalX::Inner(arg_0) => Self::Inner(arg_0),
                }
            }
        }

        pub enum JfyiX { }

        impl ::std::convert::From<JfyiX> for X {
            fn from(jfyi: JfyiX) -> Self {
                match jfyi { }
            }
        }

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

            impl ::std::convert::From<FatalX> for X {
                fn from(fatal: FatalX) -> Self {
                    match fatal {
                        FatalX::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

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

            impl ::std::convert::From<FatalX> for X {
                fn from(fatal: FatalX) -> Self {
                    match fatal {
                        FatalX::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            pub enum JfyiX { }

            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

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

            impl ::std::convert::From<RenamedFatalXRenamed> for X {
                fn from(fatal: RenamedFatalXRenamed) -> Self {
                    match fatal {
                        RenamedFatalXRenamed::Inner(arg_0) => Self::Inner(arg_0),
                    }
                }
            }

            #[derive(::std::fmt::Debug, ::thiserror::Error)]
            pub enum JfyiX { }

            impl ::std::convert::From<JfyiX> for X {
                fn from(jfyi: JfyiX) -> Self {
                    match jfyi { }
                }
            }

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
