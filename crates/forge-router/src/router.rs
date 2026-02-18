use std::collections::HashMap;

use super::BoxedHandler;
use super::RouterError;
use forge_http::HttpMethod;
use forge_utils::{PathMatch, PathTree, Segment};

type Path = &'static str;
type Routes<T> = HashMap<HttpMethod, PathTree<BoxedHandler<T>>>;

const ROUTER_RULES: (char, char) = ('/', ':');

pub struct Routable<T> {
    pub path: &'static str,
    pub method: HttpMethod,
    pub make: fn() -> BoxedHandler<T>,
}

pub struct Route<T> {
    pub path: Path,
    pub method: HttpMethod,
    pub handler: BoxedHandler<T>,
}

pub struct Router<T> {
    routes: Routes<T>,
}

impl<T> Default for Router<T>
where
    T: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Router<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self { routes: HashMap::new() }
    }

    pub fn register<F>(&mut self, routable: F)
    where
        F: FnOnce() -> Routable<T>,
    {
        let routable: Routable<T> = routable();

        self.add_route(Route {
            path: routable.path,
            method: routable.method,
            handler: (routable.make)(),
        })
        .unwrap_or_else(|e: RouterError| panic!("failed to register route {e}"));
    }

    pub fn get_route<'a, 'b>(
        &'a self,
        path: &'b str,
        method: &HttpMethod,
    ) -> Option<PathMatch<'a, 'b, BoxedHandler<T>>> {
        let path_tree: &PathTree<BoxedHandler<T>> = self.routes.get(method)?;
        path_tree.find(Self::sanitize_path(path))
    }

    fn add_route(&mut self, route: Route<T>) -> Result<(), RouterError> {
        let path_tree: &mut PathTree<BoxedHandler<T>> = self.routes.entry(route.method).or_default();

        if path_tree
            .insert(Self::parse_to_segment(route.path), route.handler)
            .is_some()
        {
            return Err(RouterError::DuplicateRoute(Self::fmt_route(&route.method, route.path)));
        };

        Ok(())
    }

    fn parse_to_segment<'a>(path: &'a str) -> impl Iterator<Item = Segment<'a>> {
        Self::sanitize_path(path).map(|path: &str| {
            if path.starts_with(ROUTER_RULES.1) {
                Segment::Param(&path[1..])
            } else {
                Segment::Exact(path)
            }
        })
    }

    fn sanitize_path(path: &str) -> impl Iterator<Item = &str> {
        path.trim_matches(ROUTER_RULES.0)
            .split(ROUTER_RULES.0)
            .filter(|s: &&str| !s.is_empty())
    }

    fn fmt_route(method: &HttpMethod, path: &str) -> String {
        format!("[{method}] - \"{path}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_http::{HttpMethod, HttpStatus, Response};
    use forge_macros::get;

    struct State;
    type Match<'a, 'b> = PathMatch<'a, 'b, BoxedHandler<State>>;
    type Route<'a, 'b> = Option<Match<'a, 'b>>;

    #[test]
    fn test_basic_static_route_match() {
        let mut router: Router<State> = Router::new();

        #[get("/ping")]
        async fn ping_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(ping_handler);

        let result: Route = router.get_route("/ping", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: Match = result.unwrap();
        assert!(match_data.params.is_empty());
    }

    #[test]
    fn test_route_not_found() {
        let mut router: Router<State> = Router::new();

        #[get("/ping")]
        async fn ping_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(ping_handler);

        let result: Route = router.get_route("/pong", &HttpMethod::GET);
        assert!(result.is_none());
    }

    #[test]
    fn test_method_mismatch() {
        let mut router: Router<State> = Router::new();

        #[get("/data")]
        async fn data_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(data_handler);

        let result_get: Route = router.get_route("/data", &HttpMethod::GET);
        assert!(result_get.is_some());

        let result_post: Route = router.get_route("/data", &HttpMethod::POST);
        assert!(result_post.is_none());
    }

    #[test]
    fn test_single_parameter_extraction() {
        let mut router: Router<State> = Router::new();

        #[get("/users/:id")]
        async fn users_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(users_handler);

        let result: Route = router.get_route("/users/123", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: Match = result.unwrap();
        assert_eq!(match_data.params.len(), 1);
        assert_eq!(match_data.params[0], ("id", "123"));
    }

    #[test]
    fn test_multiple_parameters_extraction() {
        let mut router: Router<State> = Router::new();

        #[get("/store/:store_id/customer/:customer_id")]
        async fn store_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(store_handler);

        let result: Route = router.get_route("/store/99/customer/500", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: Match = result.unwrap();
        assert_eq!(match_data.params.len(), 2);

        assert!(match_data.params.contains(&("store_id", "99")));
        assert!(match_data.params.contains(&("customer_id", "500")));
    }

    #[test]
    fn test_path_sanitization_and_trailing_slashes() {
        let mut router: Router<State> = Router::new();

        #[get("/api/v1/status")]
        async fn status_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(status_handler);

        let paths_to_test: Vec<&str> = vec![
            "/api/v1/status",
            "api/v1/status",
            "/api/v1/status/",
            "//api/v1/status//",
        ];

        for path in paths_to_test {
            let result: Route = router.get_route(path, &HttpMethod::GET);
            assert!(result.is_some(), "Failed to match path: {path}");
        }
    }

    #[test]
    fn test_deep_nested_static_routes() {
        let mut router: Router<State> = Router::new();

        #[get("/a/b/c/d")]
        async fn deep_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(deep_handler);

        let result: Route = router.get_route("/a/b/c/d", &HttpMethod::GET);
        assert!(result.is_some());

        let partial: Route = router.get_route("/a/b/c", &HttpMethod::GET);
        assert!(partial.is_none());
    }

    #[test]
    fn test_mixed_exact_and_param_segments() {
        let mut router: Router<State> = Router::new();

        #[get("/files/:type/recent")]
        async fn files_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(files_handler);
        let result: Route = router.get_route("/files/images/recent", &HttpMethod::GET);

        assert!(result.is_some());
        assert_eq!(result.unwrap().params[0], ("type", "images"));

        let result_fail: Route = router.get_route("/files/images/old", &HttpMethod::GET);
        assert!(result_fail.is_none());
    }

    #[test]
    #[should_panic(expected = "failed to register route [GET] - \"/duplicate\": duplicate route")]
    fn test_duplicate_route_panics() {
        let mut router: Router<State> = Router::new();

        #[get("/duplicate")]
        async fn duplicate_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(duplicate_handler);
        router.register(duplicate_handler);
    }

    #[test]
    fn test_overlapping_routes_precedence() {
        let mut router: Router<State> = Router::new();

        #[get("/users/all")]
        async fn users_all_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        #[get("/users/:id")]
        async fn users_id_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok)
        }

        router.register(users_id_handler);
        router.register(users_all_handler);

        let exact_match: Route = router.get_route("/users/all", &HttpMethod::GET);
        assert!(exact_match.is_some());
        assert!(exact_match.unwrap().params.is_empty());

        let param_match: Route = router.get_route("/users/123", &HttpMethod::GET);
        assert!(param_match.is_some());
        assert_eq!(param_match.unwrap().params[0], ("id", "123"));
    }
}
