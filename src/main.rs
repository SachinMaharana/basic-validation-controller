use actix_web::{get, http, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::anyhow;
use anyhow::Result;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
use kube::api::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    DynamicObject,
};
// use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
use rustls::internal::pemfile::{certs, rsa_private_keys};
use rustls::{NoClientAuth, ServerConfig};
use serde::Deserialize;
use serde_json::Value;
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
        .header(http::header::CONTENT_TYPE, "application/json")
        .json("message: i am chugging along just fine!")
}

#[post("/mutate")]
async fn handle_mutate(
    reqst: HttpRequest,
    body: web::Json<AdmissionReview<DynamicObject>>,
) -> impl Responder {
    info!(
        "Request Recieved: Method={:?}, URL={}",
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
            info!("{}", e.to_string());
            return HttpResponse::InternalServerError().json(e.to_string());
        }
    };

    info!("registries {:?}", whitelisted_registries);

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
            info!("LOG HERE {}: {}: {}: >", reg, pattern, image_name);
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
                "image {} is bloacklisted against {:?}",
                image_name, whitelisted_registries
            );
            resp.allowed = false;
            resp.result = Status {
                code: 403.into(),
                message: Some(format!(
                    "{} image comes from an untrusted registry! Only images from {:?} are allowed",
                    image_name, whitelisted_registries
                )),
                ..Default::default()
            };
            break;
        }
    }
    return HttpResponse::Ok().json(resp.into_review());
}

fn get_image_name(container: &Value) -> Option<&str> {
    let image_name = match container.get("image") {
        Some(image_name) => {
            let image_name = image_name.as_str();
            image_name?;
            image_name.unwrap()
        }
        None => return None,
    };
    Some(image_name)
}

fn get_containers(pod: &Value) -> Option<&Vec<Value>> {
    let containers = match pod.get("containers") {
        Some(containers) => match containers.as_array() {
            Some(container) => container,
            None => return None,
        },
        None => return None,
    };
    Some(containers)
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
    let mut config = ServerConfig::new(NoClientAuth::new());
    let cert_file = &mut BufReader::new(File::open("./certs/serverCert.pem")?);
    let key_file = &mut BufReader::new(File::open("./certs/serverKey.pem")?);
    let cert_chain = certs(cert_file).expect("error in cert");
    let mut keys = rsa_private_keys(key_file).expect("error in key");
    config.set_single_cert(cert_chain, keys.remove(0))?;

    HttpServer::new(|| App::new().service(handle_mutate).service(health))
        .bind_rustls("0.0.0.0:8443", config)?
        .run()
        .await?;
    Ok(())
}
