#[macro_use]
extern crate rocket;
extern crate serde;

use reaper_lib::bottomup::{get_fields, generate_abstract_queries};
use reaper_lib::sql::create_table;
use reaper_lib::stun::synthesize_pred;
use reaper_lib::types::*;
use rocket::fs::{relative, FileServer};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Example {
    input: Vec<ConcTable>,
    output: ConcTable,
    constants: Vec<isize>,
}

#[post("/synth", format = "json", data = "<example>")]
fn synth(example: Json<Example>) {
    let conn = create_table(&example.input).unwrap();
    for depth in 1..=4 {
        println!("Depth: {}", depth);
        let queries = generate_abstract_queries(
            (example.input.clone(), example.output.clone()),
            depth,
            &conn,
        );
        println!("{}", "looking for predicate...");
        for query in queries.iter() {
            let predicate = synthesize_pred(
                query,
                &example.output,
                &conn,
                &get_fields(query),
                &example.constants,
                3,
            );
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
