# Changes

### Added

* Add helper function for executing futures `test::block_fn()`

### Changed

* Allow to construct `Data` instances to avoid double `Arc` for `Send + Sync` types.

### Fixed

* Fix `NormalizePath` middleware impl #806


## [1.0.0-beta.2] - 2019-04-24

### Added

* Add raw services support via `web::service()`

* Add helper functions for reading response body `test::read_body()`

* Add support for `remainder match` (i.e "/path/{tail}*")

* Extend `Responder` trait, allow to override status code and headers.

* Store visit and login timestamp in the identity cookie #502

### Changed

* `.to_async()` handler can return `Responder` type #792

### Fixed

* Fix async web::Data factory handling


## [1.0.0-beta.1] - 2019-04-20

### Added

* Add helper functions for reading test response body,
 `test::read_response()` and test::read_response_json()`

* Add `.peer_addr()` #744

* Add `NormalizePath` middleware

### Changed

* Rename `RouterConfig` to `ServiceConfig`

* Rename `test::call_success` to `test::call_service`

* Removed `ServiceRequest::from_parts()` as it is unsafe to create from parts.

* `CookieIdentityPolicy::max_age()` accepts value in seconds

### Fixed

* Fixed `TestRequest::app_data()`


## [1.0.0-alpha.6] - 2019-04-14

### Changed

* Allow to use any service as default service.

* Remove generic type for request payload, always use default.

* Removed `Decompress` middleware. Bytes, String, Json, Form extractors
  automatically decompress payload.

* Make extractor config type explicit. Add `FromRequest::Config` associated type.


## [1.0.0-alpha.5] - 2019-04-12

### Added

* Added async io `TestBuffer` for testing.

### Deleted

* Removed native-tls support


## [1.0.0-alpha.4] - 2019-04-08

### Added

* `App::configure()` allow to offload app configuration to different methods

* Added `URLPath` option for logger

* Added `ServiceRequest::app_data()`, returns `Data<T>`

* Added `ServiceFromRequest::app_data()`, returns `Data<T>`

### Changed

* `FromRequest` trait refactoring

* Move multipart support to actix-multipart crate

### Fixed

* Fix body propagation in Response::from_error. #760


## [1.0.0-alpha.3] - 2019-04-02

### Changed

* Renamed `TestRequest::to_service()` to `TestRequest::to_srv_request()`

* Renamed `TestRequest::to_response()` to `TestRequest::to_srv_response()`

* Removed `Deref` impls

### Removed

* Removed unused `actix_web::web::md()`


## [1.0.0-alpha.2] - 2019-03-29

### Added

* rustls support

### Changed

* use forked cookie

* multipart::Field renamed to MultipartField

## [1.0.0-alpha.1] - 2019-03-28

### Changed

* Complete architecture re-design.

* Return 405 response if no matching route found within resource #538
