#[macro_use]
extern crate rocket;
extern crate serde;

use reaper_lib::bottomup::{generate_abstract_queries, get_fields};
use reaper_lib::sql::create_table;
use reaper_lib::types::*;
use rocket::fs::{relative, FileServer};
use rocket::serde::json::Json;
use rocket::serde::Deserialize;

#[derive(Deserialize)]
struct Example {
    input: Vec<ConcTable>,
    output: ConcTable,
    constants: Vec<isize>,
}

#[post("/synth", format = "json", data = "<example>")]
fn synth(example: Json<Example>) {
    let conn = create_table(&example.input).unwrap();
    for depth in 1..=3 {
        println!("Depth: {}", depth);
        let queries = generate_abstract_queries(
            (example.input.clone(), example.output.clone()),
            depth,
            &conn,
        );
        println!("looking for predicate...");
        for query in queries.iter() {
            let predicate =
                reaper_lib::synthesize(query, &example.output, &example.constants, 3, &conn)
                    .map(|ps| ps.first().expect("vec must not be empty").clone());
            println!("Predicate: {:?}", predicate);
            if predicate.is_ok() {
                println!("Found predicate: {:?}", predicate);
                return;
            }
        }
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from(relative!("/static")))
        .mount("/", routes![synth])
}
