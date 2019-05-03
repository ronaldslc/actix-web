use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use actix_http::{Error, Extensions, Payload, Response};
use actix_service::{NewService, Service, Void};
use futures::future::{ok, FutureResult};
use futures::{try_ready, Async, Future, IntoFuture, Poll};

use crate::extract::FromRequest;
use crate::request::HttpRequest;
use crate::responder::Responder;
use crate::service::{ServiceRequest, ServiceResponse};

/// Handler converter factory
pub trait Factory<T, R>: Clone
where
    R: Responder,
{
    fn call(&self, param: T) -> R;
}

impl<F, R> Factory<(), R> for F
where
    F: Fn() -> R + Clone,
    R: Responder,
{
    fn call(&self, _: ()) -> R {
        (self)()
    }
}

#[doc(hidden)]
pub struct Handler<F, T, R>
where
    F: Factory<T, R>,
    R: Responder,
{
    hnd: F,
    _t: PhantomData<(T, R)>,
}

impl<F, T, R> Handler<F, T, R>
where
    F: Factory<T, R>,
    R: Responder,
{
    pub fn new(hnd: F) -> Self {
        Handler {
            hnd,
            _t: PhantomData,
        }
    }
}

impl<F, T, R> Clone for Handler<F, T, R>
where
    F: Factory<T, R>,
    R: Responder,
{
    fn clone(&self) -> Self {
        Self {
            hnd: self.hnd.clone(),
            _t: PhantomData,
        }
    }
}

impl<F, T, R> Service for Handler<F, T, R>
where
    F: Factory<T, R>,
    R: Responder,
{
    type Request = (T, HttpRequest);
    type Response = ServiceResponse;
    type Error = Void;
    type Future = HandlerServiceResponse<<R::Future as IntoFuture>::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, (param, req): (T, HttpRequest)) -> Self::Future {
        let fut = self.hnd.call(param).respond_to(&req).into_future();
        HandlerServiceResponse {
            fut,
            req: Some(req),
        }
    }
}

pub struct HandlerServiceResponse<T> {
    fut: T,
    req: Option<HttpRequest>,
}

impl<T> Future for HandlerServiceResponse<T>
where
    T: Future<Item = Response>,
    T::Error: Into<Error>,
{
    type Item = ServiceResponse;
    type Error = Void;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.fut.poll() {
            Ok(Async::Ready(res)) => Ok(Async::Ready(ServiceResponse::new(
                self.req.take().unwrap(),
                res,
            ))),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => {
                let res: Response = e.into().into();
                Ok(Async::Ready(ServiceResponse::new(
                    self.req.take().unwrap(),
                    res,
                )))
            }
        }
    }
}

/// Async handler converter factory
pub trait AsyncFactory<T, R>: Clone + 'static
where
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    fn call(&self, param: T) -> R;
}

impl<F, R> AsyncFactory<(), R> for F
where
    F: Fn() -> R + Clone + 'static,
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    fn call(&self, _: ()) -> R {
        (self)()
    }
}

#[doc(hidden)]
pub struct AsyncHandler<F, T, R>
where
    F: AsyncFactory<T, R>,
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    hnd: F,
    _t: PhantomData<(T, R)>,
}

impl<F, T, R> AsyncHandler<F, T, R>
where
    F: AsyncFactory<T, R>,
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    pub fn new(hnd: F) -> Self {
        AsyncHandler {
            hnd,
            _t: PhantomData,
        }
    }
}

impl<F, T, R> Clone for AsyncHandler<F, T, R>
where
    F: AsyncFactory<T, R>,
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    fn clone(&self) -> Self {
        AsyncHandler {
            hnd: self.hnd.clone(),
            _t: PhantomData,
        }
    }
}

impl<F, T, R> Service for AsyncHandler<F, T, R>
where
    F: AsyncFactory<T, R>,
    R: IntoFuture,
    R::Item: Responder,
    R::Error: Into<Error>,
{
    type Request = (T, HttpRequest);
    type Response = ServiceResponse;
    type Error = Void;
    type Future = AsyncHandlerServiceResponse<R::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, (param, req): (T, HttpRequest)) -> Self::Future {
        AsyncHandlerServiceResponse {
            fut: self.hnd.call(param).into_future(),
            fut2: None,
            req: Some(req),
        }
    }
}

#[doc(hidden)]
pub struct AsyncHandlerServiceResponse<T>
where
    T: Future,
    T::Item: Responder,
{
    fut: T,
    fut2: Option<<<T::Item as Responder>::Future as IntoFuture>::Future>,
    req: Option<HttpRequest>,
}

impl<T> Future for AsyncHandlerServiceResponse<T>
where
    T: Future,
    T::Item: Responder,
    T::Error: Into<Error>,
{
    type Item = ServiceResponse;
    type Error = Void;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(ref mut fut) = self.fut2 {
            return match fut.poll() {
                Ok(Async::Ready(res)) => Ok(Async::Ready(ServiceResponse::new(
                    self.req.take().unwrap(),
                    res,
                ))),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(e) => {
                    let res: Response = e.into().into();
                    Ok(Async::Ready(ServiceResponse::new(
                        self.req.take().unwrap(),
                        res,
                    )))
                }
            };
        }

        match self.fut.poll() {
            Ok(Async::Ready(res)) => {
                self.fut2 =
                    Some(res.respond_to(self.req.as_ref().unwrap()).into_future());
                return self.poll();
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => {
                let res: Response = e.into().into();
                Ok(Async::Ready(ServiceResponse::new(
                    self.req.take().unwrap(),
                    res,
                )))
            }
        }
    }
}

/// Extract arguments from request
pub struct Extract<T: FromRequest, S> {
    config: Rc<RefCell<Option<Rc<Extensions>>>>,
    service: S,
    _t: PhantomData<T>,
}

impl<T: FromRequest, S> Extract<T, S> {
    pub fn new(config: Rc<RefCell<Option<Rc<Extensions>>>>, service: S) -> Self {
        Extract {
            config,
            service,
            _t: PhantomData,
        }
    }
}

impl<T: FromRequest, S> NewService for Extract<T, S>
where
    S: Service<Request = (T, HttpRequest), Response = ServiceResponse, Error = Void>
        + Clone,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = (Error, ServiceRequest);
    type InitError = ();
    type Service = ExtractService<T, S>;
    type Future = FutureResult<Self::Service, ()>;

    fn new_service(&self, _: &()) -> Self::Future {
        ok(ExtractService {
            _t: PhantomData,
            config: self.config.borrow().clone(),
            service: self.service.clone(),
        })
    }
}

pub struct ExtractService<T: FromRequest, S> {
    config: Option<Rc<Extensions>>,
    service: S,
    _t: PhantomData<T>,
}

impl<T: FromRequest, S> Service for ExtractService<T, S>
where
    S: Service<Request = (T, HttpRequest), Response = ServiceResponse, Error = Void>
        + Clone,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = (Error, ServiceRequest);
    type Future = ExtractResponse<T, S>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let (mut req, mut payload) = req.into_parts();
        req.set_route_data(self.config.clone());
        let fut = T::from_request(&req, &mut payload).into_future();

        ExtractResponse {
            fut,
            fut_s: None,
            req: Some((req, payload)),
            service: self.service.clone(),
        }
    }
}

pub struct ExtractResponse<T: FromRequest, S: Service> {
    req: Option<(HttpRequest, Payload)>,
    service: S,
    fut: <T::Future as IntoFuture>::Future,
    fut_s: Option<S::Future>,
}

impl<T: FromRequest, S> Future for ExtractResponse<T, S>
where
    S: Service<Request = (T, HttpRequest), Response = ServiceResponse, Error = Void>,
{
    type Item = ServiceResponse;
    type Error = (Error, ServiceRequest);

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(ref mut fut) = self.fut_s {
            return fut.poll().map_err(|_| panic!());
        }

        let item = try_ready!(self.fut.poll().map_err(|e| {
            let (req, payload) = self.req.take().unwrap();
            let req = ServiceRequest::from_parts(req, payload);
            (e.into(), req)
        }));

        self.fut_s = Some(self.service.call((item, self.req.take().unwrap().0)));
        self.poll()
    }
}

/// FromRequest trait impl for tuples
macro_rules! factory_tuple ({ $(($n:tt, $T:ident)),+} => {
    impl<Func, $($T,)+ Res> Factory<($($T,)+), Res> for Func
    where Func: Fn($($T,)+) -> Res + Clone,
          Res: Responder,
    {
        fn call(&self, param: ($($T,)+)) -> Res {
            (self)($(param.$n,)+)
        }
    }

    impl<Func, $($T,)+ Res> AsyncFactory<($($T,)+), Res> for Func
    where Func: Fn($($T,)+) -> Res + Clone + 'static,
          Res: IntoFuture,
          Res::Item: Responder,
          Res::Error: Into<Error>,
    {
        fn call(&self, param: ($($T,)+)) -> Res {
            (self)($(param.$n,)+)
        }
    }
});

#[rustfmt::skip]
mod m {
    use super::*;

factory_tuple!((0, A));
factory_tuple!((0, A), (1, B));
factory_tuple!((0, A), (1, B), (2, C));
factory_tuple!((0, A), (1, B), (2, C), (3, D));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I));
factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J));
}
