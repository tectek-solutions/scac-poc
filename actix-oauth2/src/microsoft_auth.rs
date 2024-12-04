use actix_web::web;
use diesel::deserialize;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::{error::Error, process::id};

use crate::{
    model::AppState,
    google_auth::OAuthResponse,
};

#[derive(Deserialize)]
pub struct MicrosoftUserResult {
    pub id: String,
    pub mail: String,
    pub displayName: String,
    pub givenName: String,
    // pub picture: String,
}

pub async fn request_microsoft_token(
    authorization_code: &str,
    data: &web::Data<AppState>,
) -> Result<OAuthResponse, Box<dyn Error>> {
    let redirect_url = data.env.microsoft_oauth_redirect_url.to_owned();
    let client_secret = data.env.microsoft_oauth_client_secret_value.to_owned();
    let client_id = data.env.microsoft_oauth_client_id.to_owned();

    let root_url = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
    let client = Client::new();

    let params = [
        ("client_id", client_id.as_str()),
        ("scope", "openid profile email User.Read Mail.Read Mail.Send offline_access"),
        ("code", authorization_code),
        ("redirect_uri", redirect_url.as_str()),
        ("grant_type", "authorization_code"),
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

pub async fn get_microsoft_user(
    access_token: &str,
    id_token: &str,
) -> Result<MicrosoftUserResult, Box<dyn Error>> {
    let client = Client::new();
    let mut url = Url::parse("https://graph.microsoft.com/v1.0/me").unwrap();

    let response = client
        .get(url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    println!("Response: {:?}", response);

    if response.status().is_success() {
        let microsoft_user = response.json::<MicrosoftUserResult>().await?;
        Ok(microsoft_user)
    } else {
        let message = format!("An error occurred while trying to retrieve user data.|\n{:?}", response);
        Err(From::from(message))
    }
}

#[derive(Debug, Deserialize)]
pub struct DraftBody {
    pub contentType: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct DraftRecipientEmail {
    pub address: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DraftRecipient {
    pub emailAddress: DraftRecipientEmail,
}

#[derive(Debug, Deserialize)]
pub struct Draft {
    pub id: String,
    pub subject: String,
    pub body: DraftBody,
    pub toRecipients: Vec<DraftRecipient>,
}

pub async fn send_connection_mail(
    access_token: &str,
    id_token: &str,
    data: &web::Data<AppState>,
) -> Result<Draft, Box<dyn Error>> {
    let client = Client::new();
    let url = Url::parse("https://graph.microsoft.com/v1.0/me/messages").unwrap();

    let message =
r#"
{
    "body": {
        "contentType": "text",
        "content": "Bonjour, Benjamin. Je me suis bien connecté à AREA."
    },
    "subject": "AREA - Connexion",
    "toRecipients": [
        {
            "emailAddress": {
                "address": "benjamin.lauret@epitech.eu",
                "name": "Benjamin Lauret"
            }
        }
    ]
}
"#;

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Length", message.len())
        .header("Content-Type", "application/json")
        .body(message)
        .send()
        .await?;

    let status = response.status();

    let draft = response.json::<Draft>().await?;
    println!("Response: {:?}", draft);


    if status.is_success() {
        let send_url = Url::parse(format!("https://graph.microsoft.com/v1.0/me/messages/{}/send", draft.id).as_str()).unwrap();
        let send_response = client
            .post(send_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Length", "0")
            .header("Content-Type", "application/json")
            .send()
            .await?;
        println!("#######################Send Response: {:?}", send_response);
        if send_response.status().is_success() {
            Ok(draft)
        } else {
            let message = format!("An error occurred while trying to send a mail.");
            Err(From::from(message))
        }
    } else {
        let message = format!("An error occurred while trying to create a draft.");
        Err(From::from(message))
    }
}