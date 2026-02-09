use std::{future::Future, pin::Pin, sync::Arc};

use forge_http::{Request, Response};

pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
pub type BoxedHandler<T> = Box<dyn Handler<T>>;

pub trait IntoHandler<T>: Send + Sync + 'static {
    fn into_handler(self) -> BoxedHandler<T>;
}

pub struct HandlerFn<T>(pub T);

pub trait Handler<T>: Send + Sync + 'static {
    fn call<'a>(&'a self, req: Request<'a>, state: Option<Arc<T>>) -> LocalBoxFuture<'a, Response<'a>>;
}

impl<T, K> Handler<K> for HandlerFn<T>
where
    K: Send + Sync + 'static,
    T: for<'r> Fn(Request<'r>, Option<Arc<K>>) -> LocalBoxFuture<'r, Response<'r>> + Send + Sync + 'static,
{
    fn call<'a>(&'a self, req: Request<'a>, state: Option<Arc<K>>) -> LocalBoxFuture<'a, Response<'a>> {
        (self.0)(req, state)
    }
}

impl<T, K> IntoHandler<K> for T
where
    K: Send + Sync + 'static,
    T: for<'a> Fn(Request<'a>, Option<Arc<K>>) -> LocalBoxFuture<'a, Response<'a>> + Send + Sync + 'static,
{
    fn into_handler(self) -> BoxedHandler<K> {
        Box::new(HandlerFn(self))
    }
}

impl<T> IntoHandler<T> for BoxedHandler<T>
where
    T: Send + Sync + 'static,
{
    fn into_handler(self) -> BoxedHandler<T> {
        self
    }
}
