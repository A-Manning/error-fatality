enum SplitVariant {
    Fatal,
    Jfyi,
}

mod r#gen;
mod opts;

pub(crate) use r#gen::{enum_gen, struct_gen};
