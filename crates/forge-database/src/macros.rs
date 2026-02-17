#[macro_export]
macro_rules! decode {
    ($ctx:expr, $t:ty => $v:expr) => {
        $ctx.0
            .get::<usize, Option<$t>>($ctx.1)
            .map($v)
            .unwrap_or(RowValue::Null)
    };
}
