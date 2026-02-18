use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, Ident, ItemFn, LitStr, Result, Token, Type,
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

fn resolve_paths() -> (syn::Path, syn::Path) {
    let forge_found = crate_name("forge");
    let router_found = crate_name("forge-router");

    let is_forge_present: bool = matches!(forge_found, Ok(FoundCrate::Name(_)));
    let is_inside_router: bool = matches!(router_found, Ok(FoundCrate::Itself));

    let http_path: syn::Path = if is_forge_present {
        parse_quote!(::forge::forge_http)
    } else {
        parse_quote!(::forge_http)
    };

    let router_path: syn::Path = if is_inside_router {
        parse_quote!(crate)
    } else if is_forge_present {
        parse_quote!(::forge::forge_router)
    } else {
        parse_quote!(::forge_router)
    };

    (http_path, router_path)
}

fn last_path_ident(ty: &Type) -> Option<&Ident> {
    match ty {
        Type::Path(tp) => tp.path.segments.last().map(|s| &s.ident),
        _ => None,
    }
}

fn is_request_type(ty: &Type) -> bool {
    matches!(last_path_ident(ty), Some(ident) if ident == "Request")
}

fn extract_arc_inner_ty(ty: &Type) -> Option<Type> {
    let Type::Path(tp) = ty else { return None };
    let seg: &syn::PathSegment = tp.path.segments.last()?;
    if seg.ident != "Arc" {
        return None;
    }

    let syn::PathArguments::AngleBracketed(ab) = &seg.arguments else {
        return None;
    };

    if ab.args.len() != 1 {
        return None;
    }

    match ab.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner.clone()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
enum ReqPos {
    First,
    Second,
}

#[derive(Clone)]
struct InputsShape {
    has_req: bool,
    has_state: bool,
    state_ty: Option<Type>,
    req_pos: Option<ReqPos>,
}

fn parse_inputs(inputs: &syn::punctuated::Punctuated<FnArg, Token![,]>) -> Result<InputsShape> {
    if inputs.len() > 2 {
        return Err(Error::new(
            inputs.span(),
            "#[route] Handler must take (), (Request), (Arc<T>), or (Request, Arc<T>)",
        ));
    }

    let mut has_req: bool = false;
    let mut has_state: bool = false;
    let mut state_ty: Option<Type> = None;
    let mut req_pos: Option<ReqPos> = None;

    for (idx, input) in inputs.iter().enumerate() {
        let typed: &syn::PatType = match input {
            FnArg::Typed(t) => t,
            FnArg::Receiver(r) => {
                return Err(Error::new(r.span(), "#[route] Cannot be used on methods (no self)"));
            }
        };

        if is_request_type(&typed.ty) {
            if has_req {
                return Err(Error::new(typed.span(), "Duplicate Request argument"));
            }

            has_req = true;
            req_pos = Some(if idx == 0 { ReqPos::First } else { ReqPos::Second });
            continue;
        }

        if let Some(inner) = extract_arc_inner_ty(&typed.ty) {
            if has_state {
                return Err(Error::new(typed.span(), "Duplicate Arc<T> (state) argument"));
            }

            has_state = true;
            state_ty = Some(inner);
            continue;
        }

        return Err(Error::new(typed.span(), "Argument must be Request<'_> or Arc<T>"));
    }

    Ok(InputsShape {
        has_req,
        has_state,
        state_ty,
        req_pos,
    })
}

#[derive(Clone)]
enum HandlerKind {
    Generic,
    Stateful { state_ty: Box<Type> },
}

struct ExpandModel {
    func: ItemFn,
    public_name: Ident,
    inner_name: Ident,
    http_path: syn::Path,
    router_path: syn::Path,
    method_lit: LitStr,
    path_lit: LitStr,
    shape: InputsShape,
    kind: HandlerKind,
}

fn build_model(args: RouteArgs, mut func: ItemFn) -> Result<ExpandModel> {
    if func.sig.asyncness.is_none() {
        return Err(Error::new(func.sig.span(), "#[route] Requires an async fn"));
    }

    let (http_path, router_path) = resolve_paths();

    let public_name: Ident = func.sig.ident.clone();
    let inner_name: Ident = format_ident!("__forge_route_impl_{public_name}");
    func.sig.ident = inner_name.clone();

    let shape: InputsShape = parse_inputs(&func.sig.inputs)?;

    let kind: HandlerKind = match (shape.has_req, shape.has_state) {
        (false, false) | (true, false) => HandlerKind::Generic,
        (false, true) | (true, true) => {
            let Some(state_ty) = shape.state_ty.clone() else {
                return Err(Error::new(func.sig.inputs.span(), "Missing state type"));
            };

            HandlerKind::Stateful {
                state_ty: Box::new(state_ty),
            }
        }
    };

    Ok(ExpandModel {
        func,
        public_name,
        inner_name,
        http_path,
        router_path,
        method_lit: args.method,
        path_lit: args.path,
        shape,
        kind,
    })
}

fn boxed_body(m: &ExpandModel) -> quote::__private::TokenStream {
    let http_path: &syn::Path = &m.http_path;
    let inner_name: &Ident = &m.inner_name;
    let shape: &InputsShape = &m.shape;

    let require_state: quote::__private::TokenStream = quote! {
        let Some(state) = state else {
            return #http_path::Response::new(#http_path::HttpStatus::InternalServerError)
                .text("Application state is required for this route, but no state was configured");
        };
    };

    match (shape.has_req, shape.has_state) {
        (false, false) => quote! {
            let _ = (req, state);
            #inner_name().await
        },

        (true, false) => quote! {
            let _ = state;
            #inner_name(req).await
        },

        (false, true) => quote! {
            let _ = req;
            #require_state
            #inner_name(state).await
        },

        (true, true) => {
            let req_first: bool = matches!(shape.req_pos, Some(ReqPos::First));

            let args: quote::__private::TokenStream = if req_first {
                quote! { req, state }
            } else {
                quote! { state, req }
            };

            quote! {
                #require_state
                #inner_name(#args).await
            }
        }
    }
}

fn expand_generic(m: &ExpandModel, body: quote::__private::TokenStream) -> quote::__private::TokenStream {
    let func: &ItemFn = &m.func;
    let public_name: &Ident = &m.public_name;
    let http_path: &syn::Path = &m.http_path;
    let router_path: &syn::Path = &m.router_path;
    let method_lit: &LitStr = &m.method_lit;
    let path_lit: &LitStr = &m.path_lit;

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
                        #body
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
}

fn expand_stateful(
    m: &ExpandModel,
    state_ty: &Type,
    body: quote::__private::TokenStream,
) -> quote::__private::TokenStream {
    let func: &ItemFn = &m.func;
    let public_name: &Ident = &m.public_name;
    let http_path: &syn::Path = &m.http_path;
    let router_path: &syn::Path = &m.router_path;
    let method_lit: &LitStr = &m.method_lit;
    let path_lit: &LitStr = &m.path_lit;

    quote! {
        #func
        pub fn #public_name() -> #router_path::Routable<#state_ty> {
            fn make() -> #router_path::handler::BoxedHandler<#state_ty> {
                fn boxed<'a>(
                    req: #http_path::Request<'a>,
                    state: ::core::option::Option<::std::sync::Arc<#state_ty>>,
                ) -> #router_path::handler::LocalBoxFuture<'a, #http_path::Response<'a>> {
                    ::std::boxed::Box::pin(async move {
                        #body
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

fn expand(m: ExpandModel) -> TokenStream {
    let body: quote::__private::TokenStream = boxed_body(&m);

    let out: quote::__private::TokenStream = match &m.kind {
        HandlerKind::Generic => expand_generic(&m, body),
        HandlerKind::Stateful { state_ty } => expand_stateful(&m, state_ty, body),
    };

    out.into()
}

#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args: RouteArgs = parse_macro_input!(attr as RouteArgs);
    let func: ItemFn = parse_macro_input!(item as ItemFn);

    match build_model(args, func) {
        Ok(model) => expand(model),
        Err(e) => e.to_compile_error().into(),
    }
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
