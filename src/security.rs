use std::future::{ready, Ready};

use actix_web::{
    body::EitherBody,
    dev::{self, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures::future::LocalBoxFuture;
// use futures_util::future::LocalBoxFuture;

pub struct CheckOrigin {
    addr: String,
}

impl CheckOrigin {
    pub fn new(host: impl ToString) -> Self {
        Self {
            addr: host.to_string(),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for CheckOrigin
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckOriginMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckOriginMiddleware {
            service,
            addr: self.addr.clone(),
        }))
    }
}
pub struct CheckOriginMiddleware<S> {
    service: S,
    addr: String,
}

impl<S, B> Service<ServiceRequest> for CheckOriginMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        // Don't forward to `/login` if we are already on `/login`.
        if !request
            .peer_addr()
            .unwrap()
            .ip()
            .to_string()
            .starts_with(&self.addr)
        {
            let (request, _pl) = request.into_parts();

            let response = HttpResponse::Unauthorized()
                .body("Go away")
                // constructed responses map to "right" body
                .map_into_right_body();

            return Box::pin(async { Ok(ServiceResponse::new(request, response)) });
        }

        let res = self.service.call(request);

        Box::pin(async move {
            // forwarded responses map to "left" body
            res.await.map(ServiceResponse::map_into_left_body)
        })
    }
}
