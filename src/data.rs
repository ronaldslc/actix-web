use std::ops::Deref;
use std::sync::Arc;

use actix_http::error::{Error, ErrorInternalServerError};
use actix_http::Extensions;
use futures::{Async, Future, IntoFuture, Poll};

use crate::dev::Payload;
use crate::extract::FromRequest;
use crate::request::HttpRequest;

/// Application data factory
pub(crate) trait DataFactory {
    fn construct(&self) -> Box<DataFactoryResult>;
}

pub(crate) trait DataFactoryResult {
    fn poll_result(&mut self, extensions: &mut Extensions) -> Poll<(), ()>;
}

/// Application data.
///
/// Application data is an arbitrary data attached to the app.
/// Application data is available to all routes and could be added
/// during application configuration process
/// with `App::data()` method.
///
/// Application data could be accessed by using `Data<T>`
/// extractor where `T` is data type.
///
/// **Note**: http server accepts an application factory rather than
/// an application instance. Http server constructs an application
/// instance for each thread, thus application data must be constructed
/// multiple times. If you want to share data between different
/// threads, a shareable object should be used, e.g. `Send + Sync`. Application
/// data does not need to be `Send` or `Sync`. Internally `Data` type
/// uses `Arc`. if your data implements `Send` + `Sync` traits you can
/// use `web::Data::new()` and avoid double `Arc`.
///
/// If route data is not set for a handler, using `Data<T>` extractor would
/// cause *Internal Server Error* response.
///
/// ```rust
/// use std::sync::Mutex;
/// use actix_web::{web, App};
///
/// struct MyData {
///     counter: usize,
/// }
///
/// /// Use `Data<T>` extractor to access data in handler.
/// fn index(data: web::Data<Mutex<MyData>>) {
///     let mut data = data.lock().unwrap();
///     data.counter += 1;
/// }
///
/// fn main() {
///     let data = web::Data::new(Mutex::new(MyData{ counter: 0 }));
///
///     let app = App::new()
///         // Store `MyData` in application storage.
///         .data(data.clone())
///         .service(
///             web::resource("/index.html").route(
///                 web::get().to(index)));
/// }
/// ```
#[derive(Debug)]
pub struct Data<T>(Arc<T>);

impl<T> Data<T> {
    /// Create new `Data` instance.
    ///
    /// Internally `Data` type uses `Arc`. if your data implements
    /// `Send` + `Sync` traits you can use `web::Data::new()` and
    /// avoid double `Arc`.
    pub fn new(state: T) -> Data<T> {
        Data(Arc::new(state))
    }

    /// Get referecnce to inner app data.
    pub fn get_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Deref for Data<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Clone for Data<T> {
    fn clone(&self) -> Data<T> {
        Data(self.0.clone())
    }
}

impl<T> From<T> for Data<T> {
    fn from(data: T) -> Self {
        Data::new(data)
    }
}

impl<T: 'static> FromRequest for Data<T> {
    type Config = ();
    type Error = Error;
    type Future = Result<Self, Error>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        if let Some(st) = req.app_config().extensions().get::<Data<T>>() {
            Ok(st.clone())
        } else {
            log::debug!(
                "Failed to construct App-level Data extractor. \
                 Request path: {:?}",
                req.path()
            );
            Err(ErrorInternalServerError(
                "App data is not configured, to configure use App::data()",
            ))
        }
    }
}

impl<T: 'static> DataFactory for Data<T> {
    fn construct(&self) -> Box<DataFactoryResult> {
        Box::new(DataFut { st: self.clone() })
    }
}

struct DataFut<T> {
    st: Data<T>,
}

impl<T: 'static> DataFactoryResult for DataFut<T> {
    fn poll_result(&mut self, extensions: &mut Extensions) -> Poll<(), ()> {
        extensions.insert(self.st.clone());
        Ok(Async::Ready(()))
    }
}

impl<F, Out> DataFactory for F
where
    F: Fn() -> Out + 'static,
    Out: IntoFuture + 'static,
    Out::Error: std::fmt::Debug,
{
    fn construct(&self) -> Box<DataFactoryResult> {
        Box::new(DataFactoryFut {
            fut: (*self)().into_future(),
        })
    }
}

struct DataFactoryFut<T, F>
where
    F: Future<Item = T>,
    F::Error: std::fmt::Debug,
{
    fut: F,
}

impl<T: 'static, F> DataFactoryResult for DataFactoryFut<T, F>
where
    F: Future<Item = T>,
    F::Error: std::fmt::Debug,
{
    fn poll_result(&mut self, extensions: &mut Extensions) -> Poll<(), ()> {
        match self.fut.poll() {
            Ok(Async::Ready(s)) => {
                extensions.insert(Data::new(s));
                Ok(Async::Ready(()))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => {
                log::error!("Can not construct application state: {:?}", e);
                Err(())
            }
        }
    }
}

/// Route data.
///
/// Route data is an arbitrary data attached to specific route.
/// Route data could be added to route during route configuration process
/// with `Route::data()` method. Route data is also used as an extractor
/// configuration storage. Route data could be accessed in handler
/// via `RouteData<T>` extractor.
///
/// If route data is not set for a handler, using `RouteData` extractor
/// would cause *Internal Server Error* response.
///
/// ```rust
/// # use std::cell::Cell;
/// use actix_web::{web, App};
///
/// struct MyData {
///     counter: Cell<usize>,
/// }
///
/// /// Use `RouteData<T>` extractor to access data in handler.
/// fn index(data: web::RouteData<MyData>) {
///     data.counter.set(data.counter.get() + 1);
/// }
///
/// fn main() {
///     let app = App::new().service(
///         web::resource("/index.html").route(
///             web::get()
///                // Store `MyData` in route storage
///                .data(MyData{ counter: Cell::new(0) })
///                // Route data could be used as extractor configuration storage,
///                // limit size of the payload
///                .data(web::PayloadConfig::new(4096))
///                // register handler
///                .to(index)
///         ));
/// }
/// ```
pub struct RouteData<T>(Arc<T>);

impl<T> RouteData<T> {
    pub(crate) fn new(state: T) -> RouteData<T> {
        RouteData(Arc::new(state))
    }

    /// Get referecnce to inner data object.
    pub fn get_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Deref for RouteData<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Clone for RouteData<T> {
    fn clone(&self) -> RouteData<T> {
        RouteData(self.0.clone())
    }
}

impl<T: 'static> FromRequest for RouteData<T> {
    type Config = ();
    type Error = Error;
    type Future = Result<Self, Error>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        if let Some(st) = req.route_data::<T>() {
            Ok(st.clone())
        } else {
            log::debug!("Failed to construct Route-level Data extractor");
            Err(ErrorInternalServerError(
                "Route data is not configured, to configure use Route::data()",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_service::Service;

    use crate::http::StatusCode;
    use crate::test::{block_on, init_service, TestRequest};
    use crate::{web, App, HttpResponse};

    #[test]
    fn test_data_extractor() {
        let mut srv =
            init_service(App::new().data(10usize).service(
                web::resource("/").to(|_: web::Data<usize>| HttpResponse::Ok()),
            ));

        let req = TestRequest::default().to_request();
        let resp = block_on(srv.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let mut srv =
            init_service(App::new().data(10u32).service(
                web::resource("/").to(|_: web::Data<usize>| HttpResponse::Ok()),
            ));
        let req = TestRequest::default().to_request();
        let resp = block_on(srv.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_route_data_extractor() {
        let mut srv = init_service(App::new().service(web::resource("/").route(
            web::get().data(10usize).to(|data: web::RouteData<usize>| {
                let _ = data.clone();
                HttpResponse::Ok()
            }),
        )));

        let req = TestRequest::default().to_request();
        let resp = block_on(srv.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // different type
        let mut srv = init_service(
            App::new().service(
                web::resource("/").route(
                    web::get()
                        .data(10u32)
                        .to(|_: web::RouteData<usize>| HttpResponse::Ok()),
                ),
            ),
        );
        let req = TestRequest::default().to_request();
        let resp = block_on(srv.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
