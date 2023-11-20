mod docker_world;

use std::fs::read;
use actix_multipart::{Multipart};
use actix_web::{get, web, App, HttpServer, Responder, error, post};
use futures_util::StreamExt;
use crate::docker_world::{DockerWorld, DocumentFile};

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[get("/hello_typst.pdf")]
async fn typst_example() -> impl Responder {

    let example = DocumentFile::new(
        "example.typ",
        read("example.typ").expect("Failed at file reading")
        );

    let compiled = DockerWorld::new(example,vec! [], None).compile();

    match compiled {
        Ok(data) => { Ok(data) }
        Err(error) => { Err(error::ErrorBadRequest(error)) }
    }
}

#[post("/compile")]
async fn typst_compile(mut payload: Multipart) -> impl Responder {
    let mut documents = vec![];

    while let Some(item) = payload.next().await {
        let mut data= vec![];
        let filename: String;

        match item {
            Err(problem) => { return Err(error::ErrorBadRequest(problem)) }
            Ok(mut field) => {
                filename = field.name().into();
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(bytes) => {
                            data.extend::<Vec<u8>>(bytes.into());
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        documents.push(DocumentFile::new(filename.as_str(), data));
    }

    let compiled = DockerWorld::new(documents.remove(0),documents, None).compile();

    match compiled {
        Ok(data) => { Ok(data) }
        Err(error) => { Err(error::ErrorBadRequest(error)) }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(greet)
            .service(typst_example)
            .service(typst_compile)
    })
    .bind(("127.0.0.1", 80)).expect("Could not bind")
    .run()
    .await
}
