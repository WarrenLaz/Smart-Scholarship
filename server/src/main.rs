use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct FormData {
    first_name: String,
    last_name: String,
    student_id: String,
    gender: String,
    dob: String,
    college_year: String,
    total_credits: String,
    phone_number: String,
    email: String,
    password: String,
}

/// Handles form submissions from the React client
async fn submit_form(form: web::Json<FormData>) -> impl Responder {
    // Log the incoming form data
    println!("Received form data: {:?}", form);

    HttpResponse::Ok().json({
        serde_json::json!({
            "status": "success",
            "message": "Form submitted successfully",
            "data": form.into_inner()
        })
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server on http://127.0.0.1:8000");

    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .route("/submit", web::post().to(submit_form))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}

