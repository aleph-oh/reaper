#[macro_use]
extern crate rocket;

use rocket::fs::{relative, FileServer};
use rocket::serde::json::Json;

struct Example<'r> {
  name: &'r str,
}

#[post("/synth", format = "json", data = "<example>")]
fn new_user(user: Json<Example>) {
    /* ... */
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", FileServer::from(relative!("/static")))
}
