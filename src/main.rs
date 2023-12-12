#[macro_use]
extern crate rocket;
extern crate serde;

use rocket::fs::{relative, FileServer};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use reaper_lib::types::*;

#[derive(Deserialize)]
struct Example {
    input: Vec<ConcTable>,
    output: ConcTable,
    constants: Vec<i32>,
}

#[post("/synth", format = "json", data = "<example>")]
fn synth(example: Json<Example>) {
    println!("{:?}", example.input);
    println!("{:?}", example.output);
    println!("{:?}", example.constants);
    todo!();
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", FileServer::from(relative!("/static")))
        .mount("/", routes![synth])
}
