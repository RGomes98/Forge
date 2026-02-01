use std::{future, pin::Pin};

use forge_http::{Request, Response, response::IntoResponse};

pub type Result<'a> = Pin<Box<dyn Future<Output = Response<'a>> + Send + 'a>>;
pub type Handler = Box<dyn for<'a> Fn(Request<'a>) -> Result<'a> + Send + Sync>;

pub struct OutputWrapper<T>(pub Option<T>);

pub trait IntoHandler: Send + Sync + 'static {
    fn into_handler(self) -> Handler;
}

impl<T> IntoHandler for T
where
    T: for<'a> Fn(Request<'a>) -> Result<'a> + Send + Sync + 'static,
{
    fn into_handler(self) -> Handler {
        Box::new(self)
    }
}

pub trait AsyncResolver<'a> {
    type Output: Future<Output = Response<'a>> + Send;
    fn resolve(self) -> Self::Output;
}

impl<'a, T, K> AsyncResolver<'a> for OutputWrapper<K>
where
    K: Future<Output = T> + Send + 'a,
    T: IntoResponse<'a>,
{
    type Output = Pin<Box<dyn Future<Output = Response<'a>> + Send + 'a>>;

    fn resolve(mut self) -> Self::Output {
        let output: K = self.0.take().expect("\"AsyncResolver\" initialized without value");

        Box::pin(async move {
            let result: T = output.await;
            result.into_response()
        })
    }
}

pub trait SyncResolver<'a> {
    type Output: Future<Output = Response<'a>> + Send;
    fn resolve(self) -> Self::Output;
}

impl<'a, T> SyncResolver<'a> for OutputWrapper<T>
where
    T: IntoResponse<'a> + Send + 'a,
{
    type Output = Pin<Box<dyn Future<Output = Response<'a>> + Send + 'a>>;

    fn resolve(self) -> Self::Output {
        let output: T = self.0.expect("Value of \"SyncResolver\" consumed twice");
        Box::pin(future::ready(output.into_response()))
    }
}
