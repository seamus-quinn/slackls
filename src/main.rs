extern crate url;
extern crate hyper;
extern crate hyper_tls;
extern crate futures;

// use std::env;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::cell::RefCell;

use url::Url;

use hyper::{Body, Client, Request, Response, Server};
use hyper::service::{Service, NewService};
use hyper::rt::{self, Future, Stream};

use hyper_tls::HttpsConnector;

use webbrowser;

const SLACK_CLIENT_ID: &str = "Your client id";
const SLACK_CLIENT_SECRET: &str = "Your client secret";

struct SlackAuthService {
    code: Arc<Mutex<Option<String>>>,
    code_sender: RefCell<Option<futures::Complete<()>>>,
}

impl NewService for SlackAuthService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type InitError = hyper::Error;
    type Service = Self;
    type Future = Box<dyn Future<Item = Self::Service, Error = hyper::Error> + Send>;
    fn new_service(&self) -> Self::Future {
        let code_sender = if let Some(sender) = self.code_sender.replace(None) {
            RefCell::new(Some(sender))
        } else {
            RefCell::new(None)
        };
        Box::new(futures::finished(Self {
            code: Arc::clone(&self.code),
            code_sender,
        }))
    }
}

impl Service for SlackAuthService {
    type ReqBody = Body;
    type ResBody= Body;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item=Response<Self::ResBody>, Error=hyper::Error> + Send>;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        let queries: HashMap<_, _> = Url::parse(format!("http://localhost:9937{}", req.uri().path_and_query().unwrap().to_string()).as_str())
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();

        let code = queries.get("code").unwrap().clone();
        let response = Response::new(Body::from(code.clone()));

        let mut c = self.code.lock().unwrap();
        *c = Some(code);
        self.code_sender.replace(None).unwrap().send(()).unwrap();
        Box::new(futures::finished(response))
    }
}

impl SlackAuthService {
    // fn start(self, port: u16) {
    //     let address = ([127, 0, 0, 1], port).into();
    //     let server = Server::bind(&address)
    //         .serve(self)
    //         .map_err(|error| eprintln!("Server error: {}", error)) ;
    //     rt::run(server);
    // }

    fn fetch_access_token(code: Arc<Mutex<Option<String>>>) -> impl Future<Item=(), Error=()> {

        let code: String = match *code.lock().unwrap() {
            Some(ref s) => s.to_string(),
            None => panic!(),
        };

        println!("{}", code);

        let mut access_token_url = Url::parse("https://slack.com/api/oauth.access").unwrap();
        let access_token_url = access_token_url
            .query_pairs_mut()
            .append_pair("client_id", SLACK_CLIENT_ID)
            .append_pair("client_secret", SLACK_CLIENT_SECRET)
            .append_pair("code", code.as_str())
            .finish();
        let access_token_uri = access_token_url
            .to_string()
            .parse()
            .unwrap();

        let https = HttpsConnector::new(4)
            .expect("TLS initialization failed");
        let client = Client::builder()
            .build::<_, hyper::Body>(https);
        client.get(access_token_uri)
          .and_then(|res| {
              println!("Response: {}", res.status());
              println!("Headers: {:#?}", res.headers());

              res.into_body().for_each(|chunk| {
                  io::stdout().write_all(&chunk)
                              .map_err(|e| panic!("Slackls expects stdout is open, error={}", e))
              })
          })
          .map(|_| {
              println!("\n\nDone.");
          })
          .map_err(|err| {
              eprintln!("Error {}", err);
          })
    }
}

fn main() {
    // TODO: Use the clap crate for command line arguments.
    // let args: Vec<String> = env::args().collect();

    let mut authorize_url = Url::parse("https://slack.com/oauth/authorize").unwrap();
    let authorize_url = authorize_url
        .query_pairs_mut()
        .append_pair("client_id", SLACK_CLIENT_ID)
        .append_pair("scope", "client")
        .finish();

    if webbrowser::open(&authorize_url.to_string()).is_ok() {
        println!("Browser opened...");
        println!("Click on Allow");

        let (code_sender, code_receiver) = futures::sync::oneshot::channel::<()>();

        let sender = RefCell::new(Some(code_sender));
        let code: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let auth_service = SlackAuthService{code:Arc::clone(&code), code_sender: sender};

        // TODO: This is no longer really necessary, so refactor
        let finish = code_receiver.and_then(move |_| {
            Ok(())
        }).map_err(|error| eprintln!("{}", error));

        let address = ([127, 0, 0, 1], 9937).into();
        let server = Server::bind(&address)
            .serve(auth_service)
            .with_graceful_shutdown(finish)
            .map_err(|error| eprintln!("Server error: {}", error))
            .and_then(move |_| {
                let code = Arc::clone(&code);
                rt::lazy(||{
                    SlackAuthService::fetch_access_token(code)
                })
            })
            .map(|response| {
                println!("{:?}",response)
            });

        rt::run(server);
    }
}
