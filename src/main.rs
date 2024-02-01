use actix_web::http::header::ContentType;
use actix_web::{get, http, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::anyhow;
use anyhow::Result;
use kube::api::DynamicObject;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};
use kube::core::Status;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys };
use serde::Deserialize;
use serde_json::{json, Value};
use serde_with::CommaSeparator;
use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;
use tracing::{debug, error, info, warn};
use tracing_subscriber::filter::EnvFilter;

#[derive(Deserialize, Debug)]
struct Environment {
    #[serde(with = "serde_with::rust::StringWithSeparator::<CommaSeparator>")]
    whitelisted_registries: Vec<String>,
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok()
        .content_type(ContentType::json())
        .json(json!({"message": "ok"}))
}

#[post("/mutate")]
async fn handle_mutate(
    reqst: HttpRequest,
    body: web::Json<AdmissionReview<DynamicObject>>,
) -> impl Responder {
    info!(
        "request recieved: method={:?}, uri={}",
        reqst.method(),
        reqst.uri(),
    );

    if let Some(content_type) = reqst.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);
            warn!("Warn: {}, Code: {}", msg, http::StatusCode::BAD_REQUEST);
            return HttpResponse::BadRequest().json(msg);
        }
    }

    let req: AdmissionRequest<_> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(err) => {
            error!("invalid request: {}", err.to_string());
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(err.to_string()).into_review());
        }
    };

    let whitelisted_registries = match envy::from_env::<Environment>() {
        Ok(environment) => environment.whitelisted_registries,
        Err(e) => {
            error!("{}", e.to_string());
            return HttpResponse::InternalServerError().json(e.to_string());
        }
    };

    info!("whitelisted registries {:?}", whitelisted_registries);

    let mut resp = AdmissionResponse::from(&req);

    let obj = match req
        .object
        .ok_or_else(|| anyhow!("could not get object from the request body"))
    {
        Ok(obj) => obj,
        Err(e) => return HttpResponse::InternalServerError().json(e.to_string()),
    };

    let pod = &obj.data["spec"];

    let containers = match get_containers(pod).ok_or_else(|| anyhow!("could not get containers")) {
        Ok(containers) => containers,
        Err(e) => return HttpResponse::InternalServerError().json(e.to_string()),
    };

    for container in containers.iter() {
        let mut whitelisted = false;

        let image_name = match get_image_name(container)
            .ok_or_else(|| anyhow!("could not resolve image from container"))
        {
            Ok(image_name) => image_name,
            Err(e) => return HttpResponse::InternalServerError().json(e.to_string()),
        };

        for reg in &whitelisted_registries {
            let pattern = format!("{}/", reg.clone());
            info!(
                "reg, pattern, image name {}: {}: {}: >",
                reg, pattern, image_name
            );
            if image_name.starts_with(pattern.as_str()) {
                debug!(
                    "image {} is whitelisted against {:?}",
                    image_name, whitelisted_registries
                );
                whitelisted = true
            }
        }
        if !whitelisted {
            debug!(
                "image {} is blacklisted against {:?}",
                image_name, whitelisted_registries
            );
            resp.allowed = false;
            resp.result = Status {
                message: format!(
                    "{} image comes from an untrusted registry! only images from {:?} are allowed",
                    image_name, whitelisted_registries
                ),
                ..Default::default()
            };
            break;
        }
    }
    HttpResponse::Ok().json(resp.into_review())
}

fn get_image_name(container: &Value) -> Option<&str> {
    container
        .get("image")
        .and_then(|image_name| image_name.as_str())
}

fn get_containers(pod: &Value) -> Option<&Vec<Value>> {
    pod.get("containers")
        .and_then(|container| container.as_array())
}

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var(
        "RUST_LOG",
        "actix_web=warn,basic_validation_controller=debug",
    );
    let filter = EnvFilter::from_default_env();

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Started http server: 0.0.0.0:8443");
    let cert_file = &mut BufReader::new(File::open("/certs/serverCert.pem")?);
    let key_file = &mut BufReader::new(File::open("/certs/serverKey.pem")?);

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();
    let mut keys: Vec<PrivateKey> = pkcs8_private_keys(key_file)
        .unwrap()
        .into_iter()
        .map(PrivateKey)
        .collect();
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, keys.remove(0))
        .expect("error in config");

    HttpServer::new(|| App::new().service(handle_mutate).service(health))
        .bind_rustls_021("0.0.0.0:8443", config)?
        .run()
        .await?;
    Ok(())
}
