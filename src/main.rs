use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Parser;
use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    AngleBracketedGenericArguments, Attribute, Expr, ExprCall, ExprClosure, ExprPath, File,
    GenericArgument, Ident, Item, ItemStatic, PathArguments, PathSegment, ReturnType,
    StaticMutability, Token, Type, TypePath, Visibility,
};

#[derive(Parser)]
struct Args {
    #[arg(name("FILE"))]
    files: Vec<PathBuf>,
}

struct LazyStatics {
    items: Vec<LazyStatic>,
}

impl Parse for LazyStatics {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse()?)
        }
        Ok(Self { items })
    }
}

#[derive(Debug)]
struct LazyStatic {
    attrs: Vec<Attribute>,
    vis: Visibility,
    static_token: Token![static],
    _ref: Token![ref],
    ident: Ident,
    colon_token: Token![:],
    ty: Type,
    eq_token: Token![=],
    expr: Expr,
    semi_token: Token![;],
}

impl Parse for LazyStatic {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            static_token: input.parse()?,
            _ref: input.parse()?,
            ident: input.parse()?,
            colon_token: input.parse()?,
            ty: input.parse()?,
            eq_token: input.parse()?,
            expr: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

impl LazyStatic {
    fn into_lazy(self) -> Item {
        let Self {
            attrs,
            vis,
            static_token,
            _ref,
            ident,
            colon_token,
            ty,
            eq_token,
            expr,
            semi_token,
        } = self;
        ItemStatic {
            attrs,
            vis,
            static_token,
            mutability: StaticMutability::None,
            ident,
            colon_token,
            ty: Box::new(Type::Path(TypePath {
                qself: None,
                path: syn::Path {
                    leading_colon: None,
                    segments: Punctuated::from_iter([PathSegment {
                        ident: Ident::new("Lazy", Span::call_site()),
                        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            colon2_token: None,
                            lt_token: Token![<](Span::call_site()),
                            args: Punctuated::from_iter([GenericArgument::Type(ty)]),
                            gt_token: Token![>](Span::call_site()),
                        }),
                    }]),
                },
            })),
            eq_token,
            expr: Box::new(Expr::Call(ExprCall {
                attrs: Vec::new(),
                func: Box::new(Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: syn::Path {
                        leading_colon: None,
                        segments: Punctuated::from_iter([
                            PathSegment {
                                ident: Ident::new("Lazy", Span::call_site()),
                                arguments: PathArguments::None,
                            },
                            PathSegment {
                                ident: Ident::new("new", Span::call_site()),
                                arguments: PathArguments::None,
                            },
                        ]),
                    },
                })),
                paren_token: syn::token::Paren(Span::call_site()),
                args: Punctuated::from_iter([Expr::Closure(ExprClosure {
                    attrs: Vec::new(),
                    lifetimes: None,
                    constness: None,
                    movability: None,
                    asyncness: None,
                    capture: None,
                    or1_token: Token![|](Span::call_site()),
                    inputs: Punctuated::new(),
                    or2_token: Token![|](Span::call_site()),
                    output: ReturnType::Default,
                    body: Box::new(expr),
                })]),
            })),
            semi_token,
        }
        .into()
    }
}

fn get_translated(source: &str) -> anyhow::Result<Vec<Item>> {
    Ok(syn::parse_file(source)?
        .items
        .into_iter()
        .filter_map(|item| match item {
            syn::Item::Macro(m) => Some(m.mac),
            _ => None,
        })
        .filter(|it| it.path.is_ident("lazy_static"))
        .map(|it| it.tokens)
        .map(syn::parse2::<LazyStatics>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|it| it.items)
        .map(LazyStatic::into_lazy)
        .collect())
}

fn main() -> anyhow::Result<()> {
    let Args { files } = Args::parse();
    if files.is_empty() || (files.len() == 1 && files[0] == Path::new("-")) {
        eprintln!("NAME: <stdin>");
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        let out = prettyplease::unparse(&File {
            shebang: None,
            attrs: Vec::new(),
            items: get_translated(&s)?,
        });
        println!("{}", out);
    } else {
        for file in files {
            println!("NAME: {}\n", file.display());
            let out = prettyplease::unparse(&File {
                shebang: None,
                attrs: Vec::new(),
                items: get_translated(&fs::read_to_string(file)?)?,
            });
            println!("{}", out);
        }
    }
    Ok(())
}
