use crate::{api, document};
use color_eyre::Report;
use eyre::bail;
use reqwest::header::CONTENT_TYPE;
use url::Url;

pub fn query(
    client: reqwest::blocking::Client,
    uri: Url,
    query_input: String,
    filter_input: String,
) -> Result<(), Report> {
    let mut q = api::ApiQuery::new();
    q.query = Some(query_input);

    q.process_filter(filter_input);

    // Split up the JSON decoding into two steps.
    // 1.) Get the text of the body.
    let response_body = match client
        .post(uri.as_ref())
        .body::<String>(serde_json::to_string(&q).unwrap())
        .header(CONTENT_TYPE, "application/json")
        .send()
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                bail!("Request failed: {:?}", resp);
            }
            match resp.text() {
                Ok(text) => text,
                Err(e) => {
                    bail!("resp.text() failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            bail!("Send failed: {:?}", e);
        }
    };

    // 2.) Parse the results as JSON.
    match serde_json::from_str::<api::ApiResponse>(&response_body) {
        Ok(mut resp) => {
            println!(
                "Matches: {:?}",
                resp.hits
                    .iter_mut()
                    .map(|mut m| {
                        m.serialization_type = document::SerializationType::Human;
                        m.to_owned()
                    })
                    .collect::<Vec<_>>()
            );
        }
        Err(e) => {
            bail!(
                "Could not deserialize body from: {}; error: {:?}",
                response_body,
                e
            )
        }
    };
    Ok(())
}
