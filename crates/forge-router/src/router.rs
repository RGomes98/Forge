use std::collections::HashMap;

use super::RouterError;
use super::{Handler, IntoHandler};
use forge_http::HttpMethod;
use forge_utils::{PathMatch, PathTree, Segment};
use tracing::{debug, trace};

type Path = &'static str;
type Routes = HashMap<HttpMethod, PathTree<Handler>>; // TODO: Add support to dynamic routes (wildcards)

const ROUTER_RULES: (char, char) = ('/', ':');

pub struct Route {
    pub path: Path,
    pub handler: Handler,
    pub method: HttpMethod,
}

pub struct Router {
    routes: Routes,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        trace!("Initializing router");
        Self { routes: HashMap::new() }
    }

    pub fn register<T: IntoHandler>(&mut self, method: HttpMethod, path: &'static str, handler: T) {
        self.add_route(Route {
            path,
            method,
            handler: handler.into_handler(),
        })
        .expect("Fatal error registering route");
    }

    pub fn get_route<'a, 'b>(&'a self, path: &'b str, method: &HttpMethod) -> Option<PathMatch<'a, 'b, Handler>> {
        trace!("Looking up route for {method} {path}");
        let path_tree: &PathTree<Handler> = self.routes.get(method)?;
        path_tree.find(Self::sanitize_path(path))
    }

    fn add_route(&mut self, route: Route) -> Result<(), RouterError> {
        let path_tree: &mut PathTree<Handler> = self.routes.entry(route.method).or_default();

        if path_tree
            .insert(Self::parse_to_segment(route.path), route.handler)
            .is_some()
        {
            return Err(RouterError::DuplicateRoute(Self::fmt_route(&route.method, route.path)));
        };

        debug!("Registered route: {}", Self::fmt_route(&route.method, route.path));
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
    use crate::get;
    use forge_http::{HttpStatus, Request, Response};

    fn dummy_handler(_: Request) -> Response {
        Response::new(HttpStatus::Ok)
    }

    #[test]
    fn test_basic_static_route_match() {
        let mut router: Router = Router::new();
        get!(router, "/ping", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/ping", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: PathMatch<Handler> = result.unwrap();
        assert!(match_data.params.is_empty());
    }

    #[test]
    fn test_route_not_found() {
        let mut router: Router = Router::new();
        get!(router, "/ping", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/pong", &HttpMethod::GET);
        assert!(result.is_none());
    }

    #[test]
    fn test_method_mismatch() {
        let mut router: Router = Router::new();
        get!(router, "/data", dummy_handler);

        let result_get: Option<PathMatch<Handler>> = router.get_route("/data", &HttpMethod::GET);
        assert!(result_get.is_some());

        let result_post: Option<PathMatch<Handler>> = router.get_route("/data", &HttpMethod::POST);
        assert!(result_post.is_none());
    }

    #[test]
    fn test_single_parameter_extraction() {
        let mut router: Router = Router::new();
        get!(router, "/users/:id", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/users/123", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: PathMatch<Handler> = result.unwrap();
        assert_eq!(match_data.params.len(), 1);
        assert_eq!(match_data.params[0], ("id", "123"));
    }

    #[test]
    fn test_multiple_parameters_extraction() {
        let mut router: Router = Router::new();
        get!(router, "/store/:store_id/customer/:customer_id", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/store/99/customer/500", &HttpMethod::GET);
        assert!(result.is_some());

        let match_data: PathMatch<Handler> = result.unwrap();
        assert_eq!(match_data.params.len(), 2);

        let has_store: bool = match_data.params.contains(&("store_id", "99"));
        let has_customer: bool = match_data.params.contains(&("customer_id", "500"));

        assert!(has_store);
        assert!(has_customer);
    }

    #[test]
    fn test_path_sanitization_and_trailing_slashes() {
        let mut router: Router = Router::new();
        get!(router, "/api/v1/status", dummy_handler);

        let paths_to_test: Vec<&str> = vec![
            "/api/v1/status",
            "api/v1/status",
            "/api/v1/status/",
            "//api/v1/status//",
        ];

        for path in paths_to_test {
            let result: Option<PathMatch<Handler>> = router.get_route(path, &HttpMethod::GET);
            assert!(result.is_some(), "Failed to match path: {path}");
        }
    }

    #[test]
    fn test_deep_nested_static_routes() {
        let mut router: Router = Router::new();
        get!(router, "/a/b/c/d", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/a/b/c/d", &HttpMethod::GET);
        assert!(result.is_some());

        let partial: Option<PathMatch<Handler>> = router.get_route("/a/b/c", &HttpMethod::GET);
        assert!(partial.is_none());
    }

    #[test]
    fn test_mixed_exact_and_param_segments() {
        let mut router: Router = Router::new();
        get!(router, "/files/:type/recent", dummy_handler);

        let result: Option<PathMatch<Handler>> = router.get_route("/files/images/recent", &HttpMethod::GET);
        assert!(result.is_some());
        assert_eq!(result.unwrap().params[0], ("type", "images"));

        let result_fail: Option<PathMatch<Handler>> = router.get_route("/files/images/old", &HttpMethod::GET);
        assert!(result_fail.is_none());
    }

    #[test]
    #[should_panic(expected = "Fatal error registering route")]
    fn test_duplicate_route_panics() {
        let mut router: Router = Router::new();
        get!(router, "/duplicate", dummy_handler);
        get!(router, "/duplicate", dummy_handler);
    }

    #[test]
    fn test_overlapping_routes_precedence() {
        let mut router: Router = Router::new();

        get!(router, "/users/all", dummy_handler);
        get!(router, "/users/:id", dummy_handler);

        let exact_match: Option<PathMatch<Handler>> = router.get_route("/users/all", &HttpMethod::GET);
        assert!(exact_match.is_some());
        assert!(exact_match.unwrap().params.is_empty());

        let param_match: Option<PathMatch<Handler>> = router.get_route("/users/123", &HttpMethod::GET);
        assert!(param_match.is_some());
        assert_eq!(param_match.unwrap().params[0], ("id", "123"));
    }
}
