use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, query, query_as};
use dotenvy::dotenv;
use std::{env, string};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{NaiveDate, Utc}; 
use chrono::Datelike;

#[derive(Serialize, Deserialize, Debug)]
struct FormData {
    first_name: String,
    last_name: String,
    student_id: String,
    gender: String,
    dob: String,
    college_year: String,
    total_credits: i32,
    phone_number: String,
    email: String,
    password: Option<String>,// Password can be NULL in the database query
    role: i16, // Role as i16, ensure SQL query casts it correctly
    gpa: f32, 
}



#[derive(Serialize, Deserialize, Debug)]
struct UpdateStatusData {
    student_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct LoginData {
    email: String,
    password: Option<String>,
}


#[derive(Serialize, Deserialize, Debug)]
struct UserData {
    email: String,
    status: Option<i64>,
    role : Option<i64>
}

#[derive(Serialize, Deserialize, Debug)]
struct Response {
    first_name: String,
    last_name: String,
    student_id: String,
    gender: String,
    dob: String,
    college_year: String,
    total_credits: Option<i64>,
    phone_number: String,
    email: String,
    role: Option<i64>,
    status: Option<i64>,
    gpa: Option<f64>
}


fn calculate_age(dob: &str) -> Option<i32> {
    if let Ok(birth_date) = NaiveDate::parse_from_str(dob, "%Y-%m-%d") {
        let current_date = Utc::now().naive_utc().date();
        let mut age = current_date.year() - birth_date.year();

        // Adjust age if the birthday hasn't occurred yet this year
        if current_date.month() < birth_date.month() || 
           (current_date.month() == birth_date.month() && current_date.day() < birth_date.day()) {
            age -= 1;
        }

        Some(age)
    } else {
        None // Return None if the date is not valid
    }
}

fn calculate_eligibility(gpa: f32, credit_hours: i32, age: i32) -> i32 {
    if gpa <= 3.2 || credit_hours <= 12 || age <= 23 {
        1 // ineligible
    } else {
        0 // eligible (default)
    }
}


async fn update_applicant_status(
    status_data: web::Json<UpdateStatusData>, 
    db_pool: web::Data<SqlitePool>
) -> impl Responder {
    let student_id = &status_data.student_id;

    // Update the applicant's status to 2 (Accepted)
    match query!(
        r#"
        UPDATE form_data 
        SET status = 2
        WHERE student_id = ?
        "#,
        student_id
    )
    .execute(db_pool.get_ref())
    .await 
    {
        Ok(_) => HttpResponse::Ok().json({
            serde_json::json!({
                "status": "success",
                "message": "Applicant status updated to accepted"
            })
        }),
        Err(error) => {
            eprintln!("Error updating applicant status: {:?}", error);
            HttpResponse::InternalServerError().json({
                serde_json::json!({
                    "status": "error",
                    "message": "Failed to update applicant status. Please try again later."
                })
            })
        }
    }
}

/// Handles form submissions and saves them to the SQLite database
async fn submit_form(form: web::Json<FormData>, db_pool: web::Data<SqlitePool>) -> impl Responder {
    let mut form_data = form.into_inner();

    let hashed_password = match hash(&form_data.password.unwrap_or_default(), DEFAULT_COST) {
        Ok(hashed) => hashed,
        Err(error) => {
            eprintln!("Error hashing password: {:?}", error);
            return HttpResponse::InternalServerError().json({
                serde_json::json!({
                    "status": "error",
                    "message": "Failed to process form due to a server error"
                })
            });
        }
    };
    form_data.password = Some(hashed_password);
    let age = calculate_age(&form_data.dob).unwrap_or(0);
    let eligibility_status = calculate_eligibility(form_data.gpa, form_data.total_credits, age);
    match query!(
        r#"
        INSERT INTO form_data 
        (first_name, last_name, student_id, gender, dob, college_year, total_credits, phone_number, email, password, status, role, gpa) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        form_data.first_name,
        form_data.last_name,
        form_data.student_id,
        form_data.gender,
        form_data.dob,
        form_data.college_year,
        form_data.total_credits,
        form_data.phone_number,
        form_data.email,
        form_data.password,
        eligibility_status, 
        form_data.role,
        form_data.gpa,
    )
    .execute(db_pool.get_ref())
    .await 
    {
        Ok(result) => {
            HttpResponse::Ok().json({
                serde_json::json!({
                    "status": "success",
                    "message": "Form submitted successfully",
                    "data_id": result.last_insert_rowid()
                })
            })
        },
        Err(error) => {
            eprintln!("Error inserting form data: {:?}", error);
            HttpResponse::InternalServerError().json({
                serde_json::json!({
                    "status": "error",
                    "message": "Failed to submit form. Please try again later."
                })
            })
        }
    }
}

/// Handles login requests and checks if the user's email and password match the database record
async fn login(form: web::Json<LoginData>, db_pool: web::Data<SqlitePool>) -> impl Responder {
    let login_data = form.into_inner();

    match query!(
        r#"
        SELECT email, password, status, role
        FROM form_data 
        WHERE email = ?
        "#,
        login_data.email
    )
    .fetch_optional(db_pool.get_ref())
    .await 
    {
        Ok(Some(user)) => {

            let is_valid_password = if let (Some(login_password), Some(user_password)) = (login_data.password, user.password) {
                match verify(&login_password, &user_password) {
                    Ok(valid) => valid,
                    Err(error) => {
                        eprintln!("Error verifying password: {:?}", error);
                        return HttpResponse::InternalServerError().json({
                            serde_json::json!({
                                "status": "error",
                                "message": "Internal server error"
                            })
                        });
                    }
                }
            } else {
                return HttpResponse::Unauthorized().json({
                    serde_json::json!({
                        "status": "error",
                        "message": "Invalid email or password"
                    })
                });
            };

            if is_valid_password {
                let user_data = UserData {
                    email: user.email,
                    status: Some(user.status),
                    role : Some(user.role)
                };

                HttpResponse::Ok().json({
                    serde_json::json!({
                        "status": "success",
                        "message": "Login successful",
                        "user": user_data
                    })
                })
            } else {
                HttpResponse::Unauthorized().json({
                    serde_json::json!({
                        "status": "error",
                        "message": "Invalid email or password"
                    })
                })
            }
        },
        Ok(None) => HttpResponse::Unauthorized().json({
            serde_json::json!({
                "status": "error",
                "message": "Invalid email or password"
            })
        }),
        Err(error) => {
            eprintln!("Error querying login data: {:?}", error);
            HttpResponse::InternalServerError().json({
                serde_json::json!({
                    "status": "error",
                    "message": "Internal server error"
                })
            })
        }
    }
}

/// Handles GET requests to retrieve all applicants from the SQLite database
async fn get_applicants(db_pool: web::Data<SqlitePool>) -> impl Responder {
    match query_as!(
        Response,
        r#"
        SELECT 
            first_name, 
            last_name, 
            student_id, 
            gender, 
            dob, 
            college_year, 
            total_credits, 
            phone_number, 
            email,
            role,
            status, 
            gpa
        FROM form_data
        "#
    )
    .fetch_all(db_pool.get_ref())
    .await 
    {
        Ok(applicants) => HttpResponse::Ok().json({
            serde_json::json!({
                "status": "success",
                "data": applicants
            })
        }),
        Err(error) => {
            eprintln!("Error fetching applicants: {:?}", error);
            HttpResponse::InternalServerError().json({
                serde_json::json!({
                    "status": "error",
                    "message": "Failed to fetch applicants. Please try again later."
                })
            })
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok(); // Load environment variables from .env

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in the .env file. Please configure it.");

    let db_pool = SqlitePool::connect(&database_url)
        .await
        .expect("Failed to connect to the SQLite database.");

    println!("Connected to SQLite database at: {}", database_url);
    println!("Starting server on http://127.0.0.1:8000");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();
        App::new()
            .app_data(web::Data::new(db_pool.clone()))
            .wrap(cors)
            .route("/submit", web::post().to(submit_form))
            .route("/login", web::post().to(login)) // New route for login
            .route("/applicants", web::get().to(get_applicants))
            .route("/applicant/update-status", web::post().to(update_applicant_status)) // New route
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
