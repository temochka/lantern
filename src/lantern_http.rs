use reqwest::{Method, Client};
use serde::{Serialize, Deserialize};

#[derive(Deserialize)]
pub struct Request {
    body: Option<String>,
    headers: Vec<(String, String)>,
    url: String,
    method: String,
}

#[derive(Serialize)]
pub struct Response {
    status: u16,
    body: Option<String>,
    headers: Vec<(String, String)>
}

pub async fn run(request: Request) -> Result<Response, Box<dyn std::error::Error>> {
    let client = Client::new();
    let mut request_builder = client.request(
        Method::from_bytes(request.method.as_bytes())?,
        &request.url
    );
    for (header, value) in request.headers {
        request_builder = request_builder.header(&header, &value);
    }
    if let Some(body) = request.body {
        request_builder = request_builder.body(body);
    }
    let resp = client.execute(request_builder.build()?).await?;
    let mut headers: Vec<(String, String)> = Vec::new();

    for (header, value) in resp.headers().iter() {
        headers.push((header.as_str().to_string(), value.to_str().unwrap_or("").to_string()));
    }

    Ok(Response { status: resp.status().as_u16(), body: resp.text().await.ok(), headers: headers })
}
