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
fn new_user(example: Json<Example>) {
    println!("Received example: {:?}", example.constants);
}

#[get("/synth2")]
fn test() {
    println!("{}", "hi jay");
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", FileServer::from(relative!("/static")))
        .mount("/", routes![new_user, test])
}
