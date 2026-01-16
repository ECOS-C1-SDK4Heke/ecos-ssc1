use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub fn generate_prelude_imports() -> TokenStream2 {
    let mut imports = TokenStream2::new();

    if cfg!(feature = "prelude") {
        imports.extend(quote! {
            #[allow(unused_imports)]
            pub use core::{
                any, arch, array, ascii, cell, char,
                concat,
                fmt::{self, Debug, Display, Formatter, Result as FmtResult, Write},
                future, ops::*,
                hash::{BuildHasher, Hash, Hasher},
                hint, iter::{self, DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator},
                marker::{self, Unpin},
                mem, num, marker::*, iter::*,
                default::*, cmp::*, clone::*,
                slice, str, task, time,
                option::Option::{self, *},
                result::Result::{self, *},
                convert::{*},
            };

            #[allow(unused_imports)]
            use core::prelude::rust_2024::derive;

            #[allow(unused_imports)]
            pub use core::{assert, assert_eq, assert_ne, debug_assert, debug_assert_eq, debug_assert_ne};
        });
    }

    if cfg!(feature = "alloc") && cfg!(feature = "prelude") {
        imports.extend(quote! {
            #[allow(unused_imports)]
            extern crate alloc;
            #[allow(unused_imports)]
            pub use alloc::{
                borrow::{Cow, ToOwned},
                boxed::Box,
                collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque},
                ffi::CString,
                format,
                rc::Rc,
                string::{String, ToString},
                sync::Arc,
                vec::Vec, vec,
            };
        });
    }

    if cfg!(feature = "hashbrown") && cfg!(feature = "hashbrown") {
        imports.extend(quote! {
            #[allow(unused_imports)]
            pub use ::ecos_ssc1::hashbrown::{
                HashMap, HashSet, TryReserveError,
                hash_map::{self, Drain, IntoIter, Iter, IterMut, Keys, Values},
                hash_set::{self, Difference, Intersection, Iter as SetIter, Union},
            };
        });
    }

    if cfg!(feature = "prelude-print") {
        imports.extend(quote! {
            pub use ecos_ssc1::{print, println};
        });
    }

    imports
}
