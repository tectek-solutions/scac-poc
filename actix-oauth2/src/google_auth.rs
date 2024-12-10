use actix_web::web::{self, Json};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;


use crate::model::AppState;

#[derive(Deserialize)]
// ! THIS STRUCT DECLARATION NEED TO BE MOVED
pub struct OAuthResponse {
    pub access_token: String,
    pub id_token: String,
}

#[derive(Deserialize)]
pub struct GoogleUserResult {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    // pub given_name: String,
    pub picture: String,
}

pub async fn request_google_token(
    authorization_code: &str,
    data: &web::Data<AppState>,
) -> Result<OAuthResponse, Box<dyn Error>> {
    let redirect_url = data.env.google_oauth_redirect_url.to_owned();
    let client_secret = data.env.google_oauth_client_secret.to_owned();
    let client_id = data.env.google_oauth_client_id.to_owned();

    let root_url = "https://oauth2.googleapis.com/token";
    let client = Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_url.as_str()),
        ("client_id", client_id.as_str()),
        ("code", authorization_code),
        ("client_secret", client_secret.as_str()),
    ];
    let response = client.post(root_url).form(&params).send().await?;

    if response.status().is_success() {
        let oauth_response = response.json::<OAuthResponse>().await?;
        Ok(oauth_response)
    } else {
        let message = "An error occurred while trying to retrieve access token.";
        Err(From::from(message))
    }
}

pub async fn get_google_user(
    access_token: &str,
    id_token: &str,
) -> Result<GoogleUserResult, Box<dyn Error>> {
    let client = Client::new();
    let mut url = Url::parse("https://www.googleapis.com/oauth2/v1/userinfo").unwrap();
    url.query_pairs_mut().append_pair("alt", "json");
    url.query_pairs_mut().append_pair("id_token", id_token);
    url.query_pairs_mut().append_pair("access_token", access_token);

    let response = client.get(url).bearer_auth(access_token).send().await?;
    if response.status().is_success() {
        let user_info = response.json::<GoogleUserResult>().await?;
        Ok(user_info)
    } else {
        let response_status = response.status();
        let response_text = response.text().await?;
        let message = format!("Error while trying to retrieve user info: {}, {}", response_status, response_text);
        Err(From::from(message))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleDraftHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleDraftPartBody {
    pub attachmentId: String,
    pub size: i32,
    pub data: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleDraftPart {
    pub partId: String,
    pub mimeType: String,
    pub filename: String,
    pub headers: Vec<GoogleDraftHeader>,
    pub body: GoogleDraftPartBody,
    pub parts: Vec<GoogleDraftPart>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleMessage {
    pub id: String,
    pub threadId: String,
    pub labelIds: Vec<String>,
    pub snippet: String,
    pub historyId: String,
    pub internalDate: String,
    pub payload: GoogleDraftPart,
    pub sizeEstimate: i32,
    pub raw: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleDraft {
    pub id: String,
    pub message: GoogleMessage,
}

pub async fn send_connection_google_mail(
    access_token: &str,
    id_token: &str,
    data: &web::Data<AppState>,
) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let url = Url::parse("https://gmail.googleapis.com/gmail/v1/users/me/messages/send").unwrap();

    let to = "benjamin.lauret@epitech.eu";
    let from = "ferd.julien@gmail.com";
    let subject = "AREA - Connection";
    let body = "Bonjour, Benjamin. Je me suis bien connecté à AREA.";

    let message = json!({
        "to": to,
        "from": from,
        "subject": subject,
        "raw": STANDARD.encode(format!(
            "From: {}\r\nTo: {}\r\nSubject: {}\r\n\r\n{}",
            from, to, subject, body
        ))
    });

    let response = client
        .post(url)
        // .bearer_auth(access_token)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Length", message.to_string().len())
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&message)?)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        let message = "An error occurred while trying to send email.";
        Err(From::from(message))
    }

}


