use crate::{
    authenticate_token::AuthenticationGuard,
    google_auth::{get_google_user, request_google_token, send_connection_google_mail},
    microsoft_auth::{get_microsoft_user, request_microsoft_token, send_connection_microsoft_mail},
    model::{AppState, LoginUserSchema, QueryCode, RegisterUserSchema, TokenClaims, User},
    response::{FilteredUser, UserData, UserResponse}
};
use actix_web::{
    cookie::{time::Duration as ActixWebDuration, Cookie},
    http::header::LOCATION,
    get, post, web, HttpResponse, Responder,
};
use chrono::{prelude::*, Duration};
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

#[get("/healthchecker")]
async fn health_checker_handler() -> impl Responder {
    const MESSAGE: &str = "How to Implement Google and GitHub OAuth2 in Rust";

    HttpResponse::Ok().json(serde_json::json!({"status": "success", "message": MESSAGE}))
}

#[post("/auth/register")]
async fn register_user_handler(
    body: web::Json<RegisterUserSchema>,
    data: web::Data<AppState>,
) -> impl Responder {
    let mut vec = data.db.lock().unwrap();

    let user = vec.iter().find(|user| user.email == body.email);

    if user.is_some() {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"status": "fail","message": "Email already exist"}));
    }

    let uuid_id = Uuid::new_v4();
    let datetime = Utc::now();

    let user = User {
        id: Some(uuid_id.to_string()),
        name: body.name.to_owned(),
        verified: false,
        email: body.email.to_owned().to_lowercase(),
        provider: "local".to_string(),
        role: "user".to_string(),
        password: "".to_string(),
        photo: "default.png".to_string(),
        createdAt: Some(datetime),
        updatedAt: Some(datetime),
    };

    vec.push(user.to_owned());

    let json_response = UserResponse {
        status: "success".to_string(),
        data: UserData {
            user: user_to_response(&user),
            id_token: "".to_string(),
            access_token: "".to_string(),
        },
    };

    HttpResponse::Ok().json(json_response)
}
#[post("/auth/login")]
async fn login_user_handler(
    body: web::Json<LoginUserSchema>,
    data: web::Data<AppState>,
) -> impl Responder {
    let vec = data.db.lock().unwrap();

    let user = vec
        .iter()
        .find(|user| user.email == body.email.to_lowercase());

    if user.is_none() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"status": "fail", "message": "Invalid email or password"}));
    }

    let user = user.unwrap().clone();

    if user.provider == "Google" {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"status": "fail", "message": "Use Google OAuth2 instead"}));
    } else if user.provider == "Microsoft" {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"status": "fail", "message": "Use Microsoft OAuth2 instead"}));
    } // Instead od adding more and more if for each oauth2 provider, make a const array of providers and check if the provider is in the array


    let jwt_secret = data.env.jwt_secret.to_owned();
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + Duration::minutes(data.env.jwt_max_age)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: user.id.unwrap(),
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .unwrap();

    let cookie = Cookie::build("token", token)
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.env.jwt_max_age, 0))
        .http_only(true)
        .finish();

    HttpResponse::Ok()
        .cookie(cookie)
        .json(serde_json::json!({"status": "success"}))
}

#[get("/sessions/oauth/google")]
async fn google_oauth_handler(
    query: web::Query<QueryCode>,
    data: web::Data<AppState>,
) -> impl Responder {
    let code = &query.code;
    let state = &query.state;

    if code.is_empty() {
        return HttpResponse::Unauthorized().json(
            serde_json::json!({"status": "fail", "message": "Authorization code not provided!"}),
        );
    }

    let token_response = request_google_token(code.as_str(), &data).await;
    if token_response.is_err() {
        let message = token_response.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail1", "message": message}));
    }

    let token_response = token_response.unwrap();
    let google_user = get_google_user(&token_response.access_token, &token_response.id_token).await;
    if google_user.is_err() {
        let message = google_user.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail2", "message": message}));
    }

    let google_user = google_user.unwrap();

    let mut vec = data.db.lock().unwrap();
    let email = google_user.email.to_lowercase();
    let user = vec.iter_mut().find(|user| user.email == email);

    let user_id: String;

    if user.is_some() {
        let user = user.unwrap();
        user_id = user.id.to_owned().unwrap();
        user.email = email.to_owned();
        user.photo = google_user.picture.clone();
        user.updatedAt = Some(Utc::now());
    } else {
        let datetime = Utc::now();
        let id = Uuid::new_v4();
        user_id = id.to_owned().to_string();
        let user_data = User {
            id: Some(id.to_string()),
            name: google_user.name.clone(),
            verified: google_user.verified_email,
            email,
            provider: "Google".to_string(),
            role: "user".to_string(),
            password: "".to_string(),
            photo: google_user.picture.clone(),
            createdAt: Some(datetime),
            updatedAt: Some(datetime),
        };

        vec.push(user_data.to_owned());
    }

    let jwt_secret = data.env.jwt_secret.to_owned();
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + Duration::minutes(data.env.jwt_max_age)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: user_id,
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .unwrap();

    let cookie = Cookie::build("token", token)
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.env.jwt_max_age, 0))
        .http_only(true)
        .finish();

    let json_response = UserResponse {
        status: "success".to_string(),
        data: UserData {
            user: FilteredUser {
                id: google_user.id,
                name: google_user.name.clone(),
                email: google_user.email,
                verified: google_user.verified_email,
                photo: google_user.picture.clone(),
                provider: "Google".to_string(),
                role: "user".to_string(),
                createdAt: Utc::now(),
                updatedAt: Utc::now(),
            },
            id_token: token_response.id_token,
            access_token: token_response.access_token,
        },
    };

    let mail_result = send_connection_google_mail(&json_response.data.access_token, &json_response.data.id_token, &data).await;
    if mail_result.is_err() {
        let message = mail_result.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail3", "message": message}));
    }

    HttpResponse::Ok()
        .cookie(cookie)
        .json(json_response)

    // let frontend_origin = data.env.client_origin.to_owned();
    // let mut response = HttpResponse::Found();
    // response.append_header((LOCATION, format!("{}{}", frontend_origin, state)));
    // response.cookie(cookie);
    // response.finish()
}

#[get("/sessions/oauth/microsoft")]
async fn microsoft_oauth_handler(
    query: web::Query<QueryCode>,
    data: web::Data<AppState>,
) -> impl Responder {
    let code = &query.code;
    let state: &String;

    if code.is_empty() {
        return HttpResponse::Unauthorized().json(
            serde_json::json!({"status": "fail", "message": "Authorization code not provided!"}),
        );
    }

    match &query.state {
        Some(value) => state = value,
        None => {
            match &query.session_state {
                Some(value) => state = value,
                None => {
                    return HttpResponse::Unauthorized().json(
                        serde_json::json!({"status": "fail", "message": "Invalid state or session_state"}),
                    );
                }
            }
        }
    }

    let token_response = request_microsoft_token(code.as_str(), &data).await;
    if token_response.is_err() {
        let message = token_response.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail1", "message": message}));
    }

    let token_response = token_response.unwrap();
    let microsoft_user = get_microsoft_user(&token_response.access_token, &token_response.id_token).await;
    if microsoft_user.is_err() {
        let message = microsoft_user.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail2", "message": message}));
    }
    let microsoft_user = microsoft_user.unwrap();

    let mut vec = data.db.lock().unwrap();
    let email = microsoft_user.mail.to_lowercase();
    let user = vec.iter_mut().find(|user| user.email == email);

    let user_id: String;

    if user.is_some() {
        let user = user.unwrap();
        user_id = user.id.to_owned().unwrap();
        user.email = email.to_owned();
        user.photo = "photo".to_string();
        user.updatedAt = Some(Utc::now());
    } else {
        let datetime = Utc::now();
        let id = Uuid::new_v4();
        user_id = id.to_owned().to_string();
        let user_data = User {
            id: Some(id.to_string()),
            name: microsoft_user.givenName.clone(),
            verified: true,
            email,
            provider: "Google".to_string(),
            role: "user".to_string(),
            password: "".to_string(),
            photo: "".to_string(),
            createdAt: Some(datetime),
            updatedAt: Some(datetime),
        };

        vec.push(user_data.to_owned());
    }

    let jwt_secret = data.env.jwt_secret.to_owned();
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + Duration::minutes(data.env.jwt_max_age)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: user_id,
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .unwrap();

    let cookie = Cookie::build("token", token)
        .path("/")
        .max_age(ActixWebDuration::new(60 * data.env.jwt_max_age, 0))
        .http_only(true)
        .finish();

    let json_response = UserResponse {
        status: "success".to_string(),
        data: UserData {
            user: FilteredUser {
                id: microsoft_user.id,
                name: microsoft_user.givenName.clone(),
                email: microsoft_user.mail,
                verified: true,
                photo: "".to_string(),
                provider: "Microsoft".to_string(),
                role: "user".to_string(),
                createdAt: Utc::now(),
                updatedAt: Utc::now(),
            },
            id_token: token_response.id_token,
            access_token: token_response.access_token,
        },
    };

    let mail_result = send_connection_microsoft_mail(&json_response.data.access_token, &json_response.data.id_token, &data).await;
    if mail_result.is_err() {
        let message = mail_result.err().unwrap().to_string();
        return HttpResponse::BadGateway()
            .json(serde_json::json!({"status": "fail3", "message": message}));
    }

    HttpResponse::Ok()
        .cookie(cookie)
        .json(json_response)

    // let frontend_origin = data.env.client_origin.to_owned();
    // let mut response = HttpResponse::Found();
    // response.append_header((LOCATION, format!("{}{}", frontend_origin, state)));
    // response.cookie(cookie);
    // response.finish()
}

#[get("/auth/logout")]
async fn logout_handler(_: AuthenticationGuard) -> impl Responder {
    let cookie = Cookie::build("token", "")
        .path("/")
        .max_age(ActixWebDuration::new(-1, 0))
        .http_only(true)
        .finish();

    HttpResponse::Ok()
        .cookie(cookie)
        .json(serde_json::json!({"status": "success"}))
}

#[get("/users/me")]
async fn get_me_handler(
    auth_guard: AuthenticationGuard,
    data: web::Data<AppState>,
) -> impl Responder {
    let vec = data.db.lock().unwrap();

    let user = vec
        .iter()
        .find(|user| user.id == Some(auth_guard.user_id.to_owned()));

    let json_response = UserResponse {
        status: "success".to_string(),
        data: UserData {
            user: user_to_response(&user.unwrap()),
            id_token: "".to_string(),
            access_token: "".to_string(),
        },
    };

    HttpResponse::Ok().json(json_response)
}

pub fn user_to_response(user: &User) -> FilteredUser {
    FilteredUser {
        id: user.id.to_owned().unwrap(),
        name: user.name.to_owned(),
        email: user.email.to_owned(),
        verified: user.verified.to_owned(),
        photo: user.photo.to_owned(),
        provider: user.provider.to_owned(),
        role: user.role.to_owned(),
        createdAt: user.createdAt.unwrap(),
        updatedAt: user.updatedAt.unwrap(),
    }
}

pub fn config(conf: &mut web::ServiceConfig) {
    let scope = web::scope("/api")
        .service(health_checker_handler)
        .service(register_user_handler)
        .service(login_user_handler)
        .service(google_oauth_handler)
        .service(microsoft_oauth_handler)
        .service(logout_handler)
        .service(get_me_handler);

    conf.service(scope);
}