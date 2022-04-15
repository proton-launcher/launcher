use std::{error::Error, thread, time::Duration};

use fancy_regex::Regex;
use reqwest::blocking::{Client, Response};
use serde_json::Value;
use web_view::Content;
use serde::{Serialize, Deserialize};

const CLIENT_ID: &'static str = "00000000402b5328";

#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub token: String,
    pub refresh: String,
    pub uuid: String,
    pub username: String,
}

trait IntoJson {
    fn into_json(self) -> Result<Value, Box<dyn Error>>;
}

impl IntoJson for Response {
    fn into_json(self) -> Result<Value, Box<dyn Error>> {
        let text = self.text()?;
        let json: Value = serde_json::from_str(&text)?;
        Ok(json)
    }
}

fn get_authorization_code_webview() -> Result<String, Box<dyn Error>> {
    let url = format!("https://login.live.com/oauth20_authorize.srf?client_id={}&redirect_uri={}&response_type={}&scope={}&state=test",
                      CLIENT_ID,
                      "https://login.live.com/oauth20_desktop.srf",
                      "code",
                      "XboxLive.signin%20offline_access");

    let web_view = web_view::builder()
        .user_data("".to_string())
        .content(Content::Url(url))
        .invoke_handler(|web_view, arg| {
            if arg.starts_with("https://login.live.com/oauth20_desktop.srf") {
                web_view.user_data_mut().push_str(arg);
                web_view.exit();
            }

            Ok(())
        })
        .build()?;

    //TODO: update on url update? instead of 4 times per second checking
    web_view.handle().dispatch(|web_view| {
        let mut web_view_running = true;
        while web_view_running {
            match web_view.eval("webkit.messageHandlers.external.postMessage(document.URL)") {
                Ok(()) => (),
                Err(e) => eprintln!("{:?}", e)
            };
            web_view_running = web_view.user_data().is_empty();
            thread::sleep(Duration::from_millis(1))
        }
        Ok(())
    })?;

    let url = web_view.run()?;

    let web_view = web_view::builder()
        .user_data(true)
        .content(Content::Url("https://login.live.com/logout.srf"))
        .invoke_handler(|web_view, arg| {
            if arg != "https://login.live.com/logout.srf" {
                *web_view.user_data_mut() = false;
                web_view.exit();
            }
            Ok(())
        })
        .debug(false)
        .visible(false)
        .build().unwrap();
    
    web_view.handle().dispatch(|web_view| {
        let mut web_view_running = true;
        while web_view_running {
            match web_view.eval("webkit.messageHandlers.external.postMessage(document.URL)") {
                 Ok(()) => (),
                Err(e) => eprintln!("{:?}", e)
            };
            web_view_running = *web_view.user_data();
             thread::sleep(Duration::from_millis(1))
         }
        Ok(())
    }).unwrap();

    web_view.run().unwrap();
    
    let regex = Regex::new("(?<=\\bcode=)([^&]*)")?;
    let code = regex.captures(url.as_str())?.ok_or("Code not found in url")?[0].to_string();

    Ok(code)
}

fn get_authorization_token(client: &Client, authorization_code: String) -> Result<(String, String), Box<dyn Error>> {
    let response = client.get(format!("https://login.live.com/oauth20_token.srf?client_id={}&code={}&grant_type={}&redirect_uri={}",
                                      CLIENT_ID,
                                      authorization_code,
                                      "authorization_code",
                                      "https://login.live.com/oauth20_desktop.srf")
                              ).send()?.into_json()?;

    Ok((response["access_token"].as_str().ok_or(format!("Invalid Authorization Token Response: {:?}", response))?.to_string(), response["refresh_token"].as_str().ok_or(format!("Invalid Authorization Token Response: {:?}", response))?.to_string()))
}

fn get_authorization_token_with_refresh(client: &Client, refresh_token: &String) -> Result<(String, String), Box<dyn Error>> {
    let response = client.get(format!("https://login.live.com/oauth20_token.srf?client_id={}&refresh_token={}&grant_type={}&redirect_uri={}",
                                      CLIENT_ID,
                                      refresh_token,
                                      "refresh_token",
                                      "https://login.live.com/oauth20_desktop.srf")
                              ).send()?.into_json()?;

    Ok((response["access_token"].as_str().ok_or(format!("Invalid Authorization Token Response: {:?}", response))?.to_string(), response["refresh_token"].as_str().ok_or(format!("Invalid Authorization Token Response: {:?}", response))?.to_string()))
}

fn authenticate_xbl(client: &Client, authorization_token: String) -> Result<String, Box<dyn Error>> {
    let response = client.post("https://user.auth.xboxlive.com/user/authenticate")
        .body(format!(r#"{{"Properties":{{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":"d={}"}},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}}"#, authorization_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()?.into_json()?;

    Ok(response["Token"].as_str().ok_or(format!("Invalid XBL Response: {:?}", response))?.to_string())
}

fn authenticate_xsts(client: &Client, xbl_token: String) -> Result<(String, String), Box<dyn Error>> {
    let response = client.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .body(format!(r#"{{"Properties":{{"SandboxId":"RETAIL","UserTokens":["{}"]}},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}}"#, xbl_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()?.into_json()?;

    Ok((
        response["Token"].as_str().ok_or(format!("Invalid XSTS response: {:?}", response))?.to_string(),
        response["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or(format!("Invalid XSTS response: {:?}", response))?.to_string()
        ))
}

fn authenticate_minecraft(client: &Client, user_hash: String, xsts_token: String) -> Result<String, Box<dyn Error>> {
    let response = client.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .body(format!(r#"{{"identityToken":"XBL3.0 x={};{}"}}"#, user_hash, xsts_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()?.into_json()?;

    Ok(response["access_token"].as_str().ok_or(format!("Invalid Minecraft response: {:?}", response))?.to_string())
}

fn get_minecraft_profile(client: &Client, access_token: &String) -> Result<(String, String), Box<dyn Error>> {
    let response = client.get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()?.into_json()?;

    Ok((
        response["id"].as_str().ok_or(format!("Invalid Minecraft Profile response: {:?}", response))?.to_string(),
        response["name"].as_str().ok_or(format!("Invalid Minecraft Profile response: {:?}", response))?.to_string(),
        ))
}

//TODO: better error handling (likely coming when gui gets implemented)
pub fn authenticate() -> Result<Profile, Box<dyn Error>> {
    let client = Client::new();
    let authorization_code = get_authorization_code_webview()?;
    let (authorization_token, refresh_token) = get_authorization_token(&client, authorization_code)?;
    let xbl_token = authenticate_xbl(&client, authorization_token)?;
    let (xsts_token, user_hash) = authenticate_xsts(&client, xbl_token)?;
    let minecraft_access_token = authenticate_minecraft(&client, user_hash, xsts_token)?;
    let (minecraft_uuid, minecraft_username) = get_minecraft_profile(&client, &minecraft_access_token)?;
    Ok(Profile {
        token: minecraft_access_token,
        refresh: refresh_token,
        uuid: minecraft_uuid,
        username: minecraft_username,
    })
}

pub fn refresh(profile: &Profile) -> Result<Profile, Box<dyn Error>> {
    let client = Client::new();
    let (authorization_token, refresh_token) = get_authorization_token_with_refresh(&client, &profile.refresh)?;
    let xbl_token = authenticate_xbl(&client, authorization_token)?;
    let (xsts_token, user_hash) = authenticate_xsts(&client, xbl_token)?;
    let minecraft_access_token = authenticate_minecraft(&client, user_hash, xsts_token)?;
    let (minecraft_uuid, minecraft_username) = get_minecraft_profile(&client, &minecraft_access_token)?;

    Ok(Profile {
        token: minecraft_access_token,
        refresh: refresh_token,
        uuid: minecraft_uuid,
        username: minecraft_username,
    })
}

pub fn validate(access_token: &String) -> Result<bool, Box<dyn Error>> {
    let client = Client::new();

    let response = client.post("https://authserver.mojang.com/validate")
        .body(format!(r#"{{"accessToken":"{}"}}"#, access_token))
        .send()?;

    Ok(response.status().as_u16() == 204)
}

/*pub fn refresh(access_token: &String) -> Result<String, Box<dyn Error>> {
    let client = Client::new();

    let response = client.post("https://authserver.mojang.com/refresh")
        .body(format!(r#"{{"accessToken":"{}","clientToken":"fsdf"}}"#, access_token))
        .send()?.into_json()?;

    Ok(response["accessToken"].as_str().ok_or(format!("Invalid Minecraft Profile response: {:?}", response))?.to_string())
}*/