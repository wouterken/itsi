use regex::Regex;
use std::{
    borrow::{Borrow, Cow},
    error::Error,
};
use syn::{Attribute, Expr, Item, Lit, LitStr, Meta};

lazy_static::lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r#"https?://[^\s'"]+"#).unwrap();
    static ref DOC_SECTION_REGEX: Regex = Regex::new(r"^@(warning|internal|private|note)\s*").unwrap();
    static ref PARAM_DIRECTIVE_REGEX: Regex = Regex::new(r"^(@\w+)\[(\S+)\]\s+(.*)$").unwrap();
    static ref OTHER_DIRECTIVE_REGEX: Regex = Regex::new(r"^(@\w+)\s+(.*)$").unwrap();
    static ref BARE_CODE_REF_REGEX: Regex = Regex::new(r"(\b)(rb_(\w|_)+)(\(|\))*").unwrap();
}

/// Append a link directive to each foreign module to the given syntax tree.
pub fn add_link_ruby_directives(
    syntax: &mut syn::File,
    link_name: &str,
    kind: &str,
) -> Result<(), Box<dyn Error>> {
    for item in syntax.items.iter_mut() {
        if let Item::ForeignMod(fmod) = item {
            fmod.attrs.push(syn::parse_quote! {
                #[link(name = #link_name, kind = #kind)]
            });
        }
    }

    Ok(())
}

/// Converts all `*const rb_encoding` and  `*const OnigEncodingTypeST` to *mut
/// _` to keep backwards compatibility with bindgen < 0.62.
pub fn ensure_backwards_compatible_encoding_pointers(syntax: &mut syn::File) {
    for item in syntax.items.iter_mut() {
        if let Item::ForeignMod(fmod) = item {
            for item in fmod.items.iter_mut() {
                if let syn::ForeignItem::Fn(f) = item {
                    if let syn::ReturnType::Type(_, ty) = &mut f.sig.output {
                        if let syn::Type::Ptr(ptr) = &mut **ty {
                            if let syn::Type::Path(path) = &*ptr.elem {
                                if path.path.segments.len() == 1
                                    && path.path.segments[0].ident == "OnigEncodingTypeST"
                                    || path.path.segments[0].ident == "rb_encoding"
                                {
                                    ptr.mutability = Some(syn::token::Mut::default());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Turn the cruby comments into rustdoc comments.
pub fn cleanup_docs(syntax: &mut syn::File, ruby_version: &str) -> Result<(), Box<dyn Error>> {
    let footer = doc_footer(ruby_version);

    for item in syntax.items.iter_mut() {
        match item {
            Item::ForeignMod(fmod) => {
                for item in fmod.items.iter_mut() {
                    match item {
                        syn::ForeignItem::Fn(f) => clean_doc_attrs(&mut f.attrs, &footer),
                        syn::ForeignItem::Static(s) => clean_doc_attrs(&mut s.attrs, &footer),
                        syn::ForeignItem::Type(s) => clean_doc_attrs(&mut s.attrs, &footer),
                        _ => {}
                    }
                }
            }
            Item::Type(t) => clean_doc_attrs(&mut t.attrs, &footer),
            Item::Struct(s) => {
                clean_doc_attrs(&mut s.attrs, &footer);

                for f in s.fields.iter_mut() {
                    clean_doc_attrs(&mut f.attrs, &footer);
                }
            }
            Item::Enum(e) => {
                clean_doc_attrs(&mut e.attrs, &footer);

                for v in e.variants.iter_mut() {
                    clean_doc_attrs(&mut v.attrs, &footer);
                }
            }
            Item::Union(u) => {
                clean_doc_attrs(&mut u.attrs, &footer);

                for u in u.fields.named.iter_mut() {
                    clean_doc_attrs(&mut u.attrs, &footer);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn clean_doc_line(attr: &mut Attribute) -> bool {
    if !attr.path().is_ident("doc") {
        return false;
    }

    let mut deprecated: bool = false;

    if let Meta::NameValue(name_value) = &mut attr.meta {
        if let Expr::Lit(expr_lit) = &mut name_value.value {
            if let Lit::Str(lit_str) = &mut expr_lit.lit {
                let cleaned = lit_str.value();
                let cleaned = cleaned.trim_matches('"').trim();
                let cleaned = URL_REGEX.replace_all(cleaned, "<${0}>");
                let cleaned =
                    DOC_SECTION_REGEX.replace_all(&cleaned, |captures: &regex::Captures| {
                        if let Some(header) = captures.get(1) {
                            format!("---\n ### {}\n", capitalize(header.as_str())).into()
                        } else {
                            Cow::Borrowed("")
                        }
                    });
                let cleaned = PARAM_DIRECTIVE_REGEX.replace(&cleaned, "- **$1** `$2` $3");
                let cleaned = OTHER_DIRECTIVE_REGEX.replace(&cleaned, "- **$1** $2");
                let mut cleaned = BARE_CODE_REF_REGEX.replace_all(&cleaned, "${1}[`${2}`]");

                if cleaned.is_empty() {
                    cleaned = "\n".into();
                }

                if cleaned.contains("@deprecated") {
                    deprecated = true;
                }

                *lit_str = LitStr::new(cleaned.borrow(), lit_str.span());
            }
        }
    }

    deprecated
}

fn clean_doc_attrs(attrs: &mut Vec<Attribute>, footer: &str) {
    let mut deprecated: bool = false;

    for attr in attrs.iter_mut() {
        if clean_doc_line(attr) {
            deprecated = true;
        };
    }

    attrs.push(syn::parse_quote! {
        #[doc = #footer]
    });

    if deprecated {
        attrs.push(syn::parse_quote! {
            #[deprecated]
        })
    }
}

fn doc_footer(ruby_version: &str) -> String {
    format!(
        "\n---\n\nGenerated by [rb-sys]({}) for Ruby {}",
        env!("CARGO_PKG_REPOSITORY"),
        ruby_version
    )
}

fn capitalize(input: &str) -> Cow<'_, str> {
    if let Some(first) = input.chars().next() {
        if first.is_ascii_uppercase() && input[1..].chars().all(|c| c.is_ascii_lowercase()) {
            Cow::Borrowed(input)
        } else {
            let mut result = String::with_capacity(input.len());
            result.push(first.to_ascii_uppercase());
            result.push_str(&input[1..].to_ascii_lowercase());
            Cow::Owned(result)
        }
    } else {
        Cow::Borrowed(input)
    }
}
