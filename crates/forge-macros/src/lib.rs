use proc_macro::TokenStream;
use proc_macro_crate::crate_name;
use quote::{format_ident, quote};
use syn::{
    Error, Ident, ItemFn, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
};

struct RouteArgs {
    path: LitStr,
    method: LitStr,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut method: Option<LitStr> = None;
        let mut path: Option<LitStr> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "method" {
                method = Some(input.parse()?);
            } else if key == "path" {
                path = Some(input.parse()?);
            } else {
                return Err(Error::new(key.span(), "Expected `method` or `path`"));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let method: LitStr = method.ok_or_else(|| Error::new(input.span(), "Missing `method=\"...\"`"))?;
        let path: LitStr = path.ok_or_else(|| Error::new(input.span(), "Missing `path=\"...\"`"))?;

        Ok(Self { method, path })
    }
}

fn resolve_paths() -> (impl quote::ToTokens, impl quote::ToTokens) {
    let has_forge_facade: bool = crate_name("forge").is_ok();
    let inside_forge_router: bool = crate_name("forge-router").is_ok();

    let http_path: syn::Path = if has_forge_facade {
        parse_quote!(::forge::forge_http)
    } else {
        parse_quote!(::forge_http)
    };

    let router_path: syn::Path = if inside_forge_router {
        parse_quote!(crate)
    } else if has_forge_facade {
        parse_quote!(::forge::forge_router)
    } else {
        parse_quote!(::forge_router)
    };

    (http_path, router_path)
}

fn extract_arc_inner_ty(ty: &syn::Type) -> Result<syn::Type> {
    let syn::Type::Path(tp) = ty else {
        return Err(Error::new(ty.span(), "Expected Arc<T> type"));
    };

    let last: &syn::PathSegment = tp
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new(ty.span(), "Bad type path"))?;

    if last.ident != "Arc" {
        return Err(Error::new(ty.span(), "Expected second argument type to be Arc<T>"));
    }

    let syn::PathArguments::AngleBracketed(ab) = &last.arguments else {
        return Err(Error::new(ty.span(), "Arc must be Arc<T>"));
    };

    if ab.args.len() != 1 {
        return Err(Error::new(ty.span(), "Arc must be Arc<T>"));
    }

    let arg: &syn::GenericArgument = ab.args.first().unwrap();
    let syn::GenericArgument::Type(inner_ty) = arg else {
        return Err(Error::new(arg.span(), "Arc must be Arc<T> (type argument)"));
    };

    Ok(inner_ty.clone())
}

#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func: ItemFn = parse_macro_input!(item);
    if func.sig.asyncness.is_none() {
        return Error::new(func.sig.span(), "#[route] Requires an async fn")
            .to_compile_error()
            .into();
    }

    let arity: usize = func.sig.inputs.len();
    if arity != 1 && arity != 2 {
        return Error::new(
            func.sig.inputs.span(),
            "#[route] Handler must take (Request) or (Request, Arc<T>)",
        )
        .to_compile_error()
        .into();
    }

    let args: RouteArgs = parse_macro_input!(attr);
    let method_lit: LitStr = args.method;
    let path_lit: LitStr = args.path;

    let public_name: Ident = func.sig.ident.clone();
    let impl_name: Ident = format_ident!("__forge_route_impl_{public_name}");
    func.sig.ident = impl_name.clone();

    let (http_path, router_path) = resolve_paths();

    let expanded: TokenStream = if arity == 1 {
        quote! {
            #func

            pub fn #public_name<T>() -> #router_path::Routable<T>
            where
                T: Send + Sync + 'static,
            {
                fn make<T>() -> #router_path::handler::BoxedHandler<T>
                where
                    T: Send + Sync + 'static,
                {
                    fn boxed<'a, T>(
                        req: #http_path::Request<'a>,
                        state: ::core::option::Option<::std::sync::Arc<T>>,
                    ) -> #router_path::handler::LocalBoxFuture<'a, #http_path::Response<'a>>
                    where
                        T: Send + Sync + 'static,
                    {
                        ::std::boxed::Box::pin(async move {
                            let _ = state;
                            #impl_name(req).await
                        })
                    }

                    <_ as #router_path::handler::IntoHandler<T>>::into_handler(boxed::<T>)
                }

                #router_path::Routable {
                    method: <#http_path::HttpMethod as ::core::str::FromStr>::from_str(#method_lit)
                        .expect("Invalid HTTP method in #[route]"),
                    path: #path_lit,
                    make: make::<T>,
                }
            }
        }
    } else {
        let second_arg_ty: &syn::Type = match func.sig.inputs.iter().nth(1).unwrap() {
            syn::FnArg::Typed(pat_ty) => &pat_ty.ty,
            syn::FnArg::Receiver(r) => {
                return Error::new(r.span(), "#[route] Cannot be used on methods (no self)")
                    .to_compile_error()
                    .into();
            }
        };

        let state_ty: syn::Type = match extract_arc_inner_ty(second_arg_ty) {
            Ok(ts) => ts,
            Err(e) => return e.to_compile_error().into(),
        };

        parse_quote! {
            #func

            pub fn #public_name() -> #router_path::Routable<#state_ty> {
                fn make() -> #router_path::handler::BoxedHandler<#state_ty> {
                    fn boxed<'a>(
                        req: #http_path::Request<'a>,
                        state: ::core::option::Option<::std::sync::Arc<#state_ty>>,
                    ) -> #router_path::handler::LocalBoxFuture<'a, #http_path::Response<'a>> {
                        ::std::boxed::Box::pin(async move {
                            let Some(state) = state else {
                                return #http_path::Response::new(#http_path::HttpStatus::InternalServerError)
                                    .text("Application state is required for this route, but no state was configured");
                            };

                            #impl_name(req, state).await
                        })
                    }

                    <_ as #router_path::handler::IntoHandler<#state_ty>>::into_handler(boxed)
                }

                #router_path::Routable {
                    method: <#http_path::HttpMethod as ::core::str::FromStr>::from_str(#method_lit)
                        .expect("Invalid HTTP method in #[route]"),
                    path: #path_lit,
                    make,
                }
            }
        }
    }
    .into();

    expanded
}

fn method_route(method: &str, attr: TokenStream, item: TokenStream) -> TokenStream {
    let path_lit: LitStr = parse_macro_input!(attr as LitStr);
    let method_lit: LitStr = LitStr::new(method, path_lit.span());
    let args: TokenStream = quote! { method = #method_lit, path = #path_lit }.into();
    route(args, item)
}

#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("GET", attr, item)
}
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("POST", attr, item)
}
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("PUT", attr, item)
}
#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("DELETE", attr, item)
}
#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("PATCH", attr, item)
}
#[proc_macro_attribute]
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("HEAD", attr, item)
}
#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_route("OPTIONS", attr, item)
}
