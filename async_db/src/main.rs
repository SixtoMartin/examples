/* Actix-Web Asynchronous Database Example

This project illustrates two examples:

    1. An asynchronous handler that executes 4 queries in *sequential order*,
       collecting the results and returning them as a single serialized json object

    2. An asynchronous handler that executes 4 queries in *parallel*,
       collecting the results and returning them as a single serialized json object

 */
use std::io;

use actix_web::{web, App, Error as AWError, HttpResponse, HttpServer, State};
use futures::future::{join_all, ok as fut_ok, Future};
use r2d2_sqlite;
use r2d2_sqlite::SqliteConnectionManager;

mod db;
use db::{Pool, Queries, WeatherAgg};

/// Version 1: Calls 4 queries in sequential order, as an asynchronous handler
fn asyncio_weather(
    db: State<Pool>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let mut result: Vec<Vec<WeatherAgg>> = vec![];

    db::execute(&db, Queries::GetTopTenHottestYears)
        .from_err()
        .and_then(move |res| {
            result.push(res);
            db::execute(&db, Queries::GetTopTenColdestYears)
                .from_err()
                .and_then(move |res| {
                    result.push(res);
                    db::execute(&db, Queries::GetTopTenHottestMonths)
                        .from_err()
                        .and_then(move |res| {
                            result.push(res);
                            db::execute(&db, Queries::GetTopTenColdestMonths)
                                .from_err()
                                .and_then(move |res| {
                                    result.push(res);
                                    fut_ok(result)
                                })
                        })
                })
        })
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
}

/// Version 2: Calls 4 queries in parallel, as an asynchronous handler
/// Returning Error types turn into None values in the response
fn parallel_weather(
    db: State<Pool>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let fut_result = vec![
        Box::new(db::execute(&db, Queries::GetTopTenHottestYears)),
        Box::new(db::execute(&db, Queries::GetTopTenColdestYears)),
        Box::new(db::execute(&db, Queries::GetTopTenHottestMonths)),
        Box::new(db::execute(&db, Queries::GetTopTenColdestMonths)),
    ];

    join_all(fut_result)
        .map_err(AWError::from)
        .map(|result| HttpResponse::Ok().json(result))
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    let sys = actix_rt::System::new("parallel_db_example");

    // Start N db executor actors (N = number of cores avail)
    let manager = SqliteConnectionManager::file("weather.db");
    let pool = Pool::new(manager).unwrap();

    // Start http server
    HttpServer::new(move || {
        App::new()
            .state(pool.clone())
            // enable logger
            // .middleware(middleware::Logger::default())
            .resource("/asyncio_weather", |r| {
                r.route(web::get().to_async(asyncio_weather))
            })
            .resource("/parallel_weather", |r| {
                r.route(web::get().to_async(parallel_weather))
            })
    })
    .bind("127.0.0.1:8080")?
    .start();

    println!("Started http server: 127.0.0.1:8080");
    sys.run()
}
