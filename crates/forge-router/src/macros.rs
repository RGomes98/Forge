#[macro_export]
macro_rules! route {
    ($router:ident, $method:expr, $path:literal, $handler:expr) => {{
        #[allow(unused_imports)]
        use $crate::{AsyncResolver, IntoResponse, SyncResolver};

        fn wrapper<'a>(req: $crate::Request<'a>) -> $crate::Result<'a> {
            Box::pin(async move { $crate::OutputWrapper(Some($handler(req))).resolve().await })
        }

        $router.register($method, $path, wrapper)
    }};
}

#[macro_export]
macro_rules! routes {
    ($router:ident, { $($method:ident $path:literal => $handler:expr),* $(,)? }) => {
        $($crate::$method!($router, $path, $handler);)*
    };
}

#[macro_export]
macro_rules! get { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::GET, $p, $($t)*) } }

#[macro_export]
macro_rules! post { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::POST, $p, $($t)*) } }

#[macro_export]
macro_rules! put { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::PUT, $p, $($t)*) } }

#[macro_export]
macro_rules! delete { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::DELETE, $p, $($t)*) } }

#[macro_export]
macro_rules! patch { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::PATCH, $p, $($t)*) } }

#[macro_export]
macro_rules! head { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::HEAD, $p, $($t)*) } }

#[macro_export]
macro_rules! options { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::OPTIONS, $p, $($t)*) } }

#[macro_export]
macro_rules! trace { ($r:ident, $p:literal, $($t:tt)*) => { $crate::route!($r, $crate::HttpMethod::TRACE, $p, $($t)*) } }
