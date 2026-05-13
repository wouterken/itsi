use std::vec;

use quote::ToTokens;

use crate::RbConfig;

const OPAQUE_STRUCTS: [&str; 4] = ["RString", "RArray", "RData", "RTypedData"];

const OPAQUE_STRUCTS_RUBY_3_3: [&str; 3] = [
    "rb_matchext_struct",
    "rb_internal_thread_event_data",
    "rb_io_internal_buffer",
];

/// Generate opaque structs for the given bindings.
pub fn opaqueify_bindings(
    rbconfig: &RbConfig,
    bindings: bindgen::Builder,
    wrapper_h: &mut String,
) -> bindgen::Builder {
    let version_specific_opaque_structs =
        get_version_specific_opaque_structs(rbconfig.major_minor());
    let structs_to_opaque = OPAQUE_STRUCTS
        .iter()
        .chain(&version_specific_opaque_structs);

    structs_to_opaque.fold(bindings, |bindings, name| {
        gen_opaque_struct(bindings, name, wrapper_h)
    })
}

/// Categorize all bindings into stable, unstable, and internal.
pub fn categorize_bindings(syntax: &mut syn::File) {
    let mut normal_items = Vec::new();
    let mut unstable_items = Vec::new();
    let mut internal_items = Vec::new();
    let mut excluded_items = Vec::new();
    let mut opaque_items = Vec::new();
    let mut opaque_idents_to_swap = Vec::new();

    for item in syntax.items.iter_mut() {
        if let syn::Item::Struct(s) = item {
            if s.ident.to_string().contains("rb_sys__Opaque__") {
                let new_name = s.ident.to_string().replace("rb_sys__Opaque__", "");
                s.ident = syn::Ident::new(&new_name, s.ident.span());
                opaque_idents_to_swap.push(new_name);

                opaque_items.push(item.clone());
            } else {
                normal_items.push(item.clone());
            }
        } else if let syn::Item::Type(t) = item {
            if t.ident.to_string().contains("rb_sys__Opaque__") {
                let new_name = t.ident.to_string().replace("rb_sys__Opaque__", "");
                t.ident = syn::Ident::new(&new_name, t.ident.span());
                opaque_idents_to_swap.push(new_name);

                opaque_items.push(item.clone());
            } else {
                normal_items.push(item.clone());
            }
        } else {
            if let syn::Item::Fn(ref mut f) = item {
                if f.sig.ident.to_string().contains("bindgen_test_") {
                    let body = &mut f.block;
                    let code = body.clone().to_token_stream().to_string();
                    let new_code = code.replace("rb_sys__Opaque__", "super::stable::");
                    let new_code = syn::parse_str::<syn::Block>(&new_code).unwrap();

                    *body = syn::parse_quote! {
                        {
                            #[allow(unused)]
                            use super::internal::*;
                            #new_code;
                        }
                    };
                }
            }

            normal_items.push(item.clone());
        }
    }

    for item in normal_items.iter_mut() {
        if let syn::Item::Type(ref mut t) = item {
            if let Ok(syn::Type::Path(ref mut type_path)) =
                syn::parse2::<syn::Type>(t.ty.to_token_stream())
            {
                if opaque_idents_to_swap.contains(&type_path.path.segments[0].ident.to_string()) {
                    let new_ident = syn::Ident::new(
                        &type_path.path.segments[0].ident.to_string(),
                        type_path.path.segments[0].ident.span(),
                    );
                    t.ty = syn::parse_quote! { crate::internal::#new_ident };
                }
            }
        }
    }

    for mut item in normal_items {
        if let syn::Item::Struct(s) = &mut item {
            if opaque_idents_to_swap.contains(&s.ident.to_string()) {
                internal_items.push(syn::Item::Struct(s.clone()));
                s.attrs.push(syn::parse_quote! {
                    #[deprecated(note = "To improve API stability with ruby-head, direct usage of Ruby internal structs has been deprecated. To migrate, please replace the usage of this internal struct with its counterpart in the `rb_sys::stable` module. For example, instead of `use rb_sys::rb_sys__Opaque__ExampleStruct;`, use `use rb_sys::stable::ExampleStruct;`. If you need to access the internals of these items, you can use the provided `rb-sys::macros` instead.")]
                });
                unstable_items.push(item);
            } else {
                excluded_items.push(item);
            }
        } else if let syn::Item::Type(t) = &mut item {
            if opaque_idents_to_swap.contains(&t.ident.to_string()) {
                internal_items.push(syn::Item::Type(t.clone()));
                t.attrs.push(syn::parse_quote! {
                    #[deprecated(note = "To improve API stability with ruby-head, direct usage of Ruby internal structs has been deprecated. To migrate, please replace the usage of this internal struct with its counterpart in the `rb_sys::stable` module. For example, instead of `use rb_sys::rb_sys__Opaque__ExampleStruct;`, use `use rb_sys::stable::ExampleStruct;`. If you need to access the internals of these items, you can use the provided `rb-sys::macros` instead.")]
                });
                unstable_items.push(item);
            } else {
                excluded_items.push(item);
            }
        } else {
            excluded_items.push(item);
        }
    }

    // Perform a pass on all std::fmt::Debug implementations to fully qualify opaque structs
    for item in excluded_items.iter_mut() {
        if let syn::Item::Impl(ref mut impl_item) = item {
            if let Some((_, syn::Path { segments, .. }, _)) = impl_item.trait_.as_ref() {
                if segments.iter().any(|segment| segment.ident == "Debug") {
                    if let syn::Type::Path(ref mut path) = *impl_item.self_ty {
                        if opaque_idents_to_swap.contains(&path.to_token_stream().to_string()) {
                            *impl_item.self_ty = syn::parse_quote! { crate::internal::#path };
                        }
                    }
                }
            }
        }
    }

    *syntax = syn::parse_quote! {
        /// Contains all items that are not yet categorized by ABI stability.
        /// These items are candidates for promotion to `stable` or `unstable`
        /// in the future.
        pub mod uncategorized {
            #(#excluded_items)*
        }

        /// Contains all items that are considered unstable ABI and should be
        /// avoided. Any items in this list offer a stable alternative for most
        /// use cases.
        pub mod unstable {
            use super::uncategorized::*;

            #(#unstable_items)*
        }

        /// Contains all items that are considered stable ABI and are safe to
        /// use. These items are intentionally opaque to prevent accidental
        /// compatibility issues.
        ///
        /// If you need to access the internals of these items, please open an
        /// issue.
        pub mod stable {
            #(#opaque_items)*
        }

        /// Unstable items for usage internally in rb_sys to avoid deprecated warnings.
        #[allow(dead_code)]
        pub (crate) mod internal {
            use super::uncategorized::*;

            #(#internal_items)*
        }
    };
}

fn gen_opaque_struct(
    bindings: bindgen::Builder,
    name: &str,
    wrapper_h: &mut String,
) -> bindgen::Builder {
    let struct_name = format!("rb_sys__Opaque__{}", name);
    wrapper_h.push_str(&format!(
        "struct {} {{ struct {} dummy; }};\n",
        struct_name, name
    ));

    bindings
        .opaque_type(&struct_name)
        .allowlist_type(struct_name)
}

fn get_version_specific_opaque_structs(major_minor: Option<(u32, u32)>) -> Vec<&'static str> {
    let Some(major_minor) = major_minor else {
        return vec![];
    };
    let mut result = vec![];
    let (major, minor) = major_minor;

    if major > 3 || (major == 3 && minor >= 3) {
        result.extend(OPAQUE_STRUCTS_RUBY_3_3)
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version_specific_opaque_structs() {
        // No version info
        assert!(get_version_specific_opaque_structs(None).is_empty());

        // Ruby 3.2.x - too old for 3.3 structs
        assert!(get_version_specific_opaque_structs(Some((3, 2))).is_empty());

        // Ruby 3.3.x - should include 3.3 structs
        assert_eq!(
            get_version_specific_opaque_structs(Some((3, 3))),
            OPAQUE_STRUCTS_RUBY_3_3.to_vec()
        );

        // Ruby 3.4.x - should include 3.3 structs
        assert_eq!(
            get_version_specific_opaque_structs(Some((3, 4))),
            OPAQUE_STRUCTS_RUBY_3_3.to_vec()
        );

        // Ruby 4.0.x - should include 3.3 structs (this was the bug!)
        assert_eq!(
            get_version_specific_opaque_structs(Some((4, 0))),
            OPAQUE_STRUCTS_RUBY_3_3.to_vec()
        );

        // Ruby 4.1.x - should include 3.3 structs
        assert_eq!(
            get_version_specific_opaque_structs(Some((4, 1))),
            OPAQUE_STRUCTS_RUBY_3_3.to_vec()
        );

        // Ruby 2.7.x - too old
        assert!(get_version_specific_opaque_structs(Some((2, 7))).is_empty());
    }
}
