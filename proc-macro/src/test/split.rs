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
                Eh,

                #[fatal]
                #[error("Explosion")]
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
            ::core::compile_error! { "splitable structs must have a source field" }
        },
    );
}

#[test]
fn regression() {
    run_test(
        quote! {
            pub enum X {
                #[fatal]
                #[error("Cancelled")]
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
