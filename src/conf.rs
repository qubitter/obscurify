use configparser::ini::Ini;
use pico_args;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone)]
pub struct Config {
    pub https: Option<HTTPSConfig>,
    pub routing: HashMap<String, (IpAddr, u16)>,
    pub service: Service,
}
#[derive(Clone)]
pub struct Service {
    pub domain: String,
    pub target: String,
    pub endpoint: String,
    pub extract: String,
    pub uri: String,
    pub redirect: String,
}

#[derive(Clone)]
pub struct HTTPSConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
}

const HELP: &str = "get a job";

pub fn parse_args_and_render_config() -> Result<Config, String> {
    let mut config = Ini::new();
    let mut pargs = pico_args::Arguments::from_env();

    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let map = config.load(match pargs.free_from_str() {
        Ok(path) => path,
        _ => String::from("./obsc.conf"),
    })?;

    let out = Config {
        https: match map.get("https") {
            Some(data) => Some(HTTPSConfig {
                cert: PathBuf::from(data.get("cert").unwrap().as_ref().unwrap().trim()),
                key: PathBuf::from(data.get("key").unwrap().as_ref().unwrap().trim()),
            }),
            None => None,
        },
        routing: match map.get("routing") {
            Some(data) => data
                .into_iter()
                .filter_map(|(key, value)| match value {
                    Some(ip) => match ip.split_once(":") {
                        Some((subip, port)) => Some((
                            key,
                            (
                                Ipv4Addr::from_str(subip).unwrap(),
                                u16::from_str_radix(port, 10),
                            ),
                        )),

                        None => match key.as_str() {
                            "http" => {
                                Some((key, (Ipv4Addr::from_str(ip).unwrap(), u16::from_str("80"))))
                            }
                            "https" => {
                                Some((key, (Ipv4Addr::from_str(ip).unwrap(), u16::from_str("443"))))
                            }
                            _ => None,
                        },
                    },
                    None => None,
                })
                .fold(HashMap::new(), move |mut a, b| {
                    a.insert(
                        b.0.to_owned(),
                        match b.1 {
                            (ip, res) => (IpAddr::V4(ip), res.unwrap()),
                        },
                    );
                    a
                }),
            None => return Err(String::from("No routing configuration!")),
        },
        service: match map.get("service") {
            Some(svc) => Service {
                domain: svc
                    .get("domain")
                    .expect("Services need domains, too!")
                    .to_owned()
                    .unwrap(),
                target: svc.get("target").expect("And targets!").to_owned().unwrap(),
                endpoint: svc
                    .get("endpoint")
                    .expect("And also endpoints")
                    .to_owned()
                    .unwrap(),
                redirect: svc
                    .get("redirect")
                    .expect("And redirects!")
                    .to_owned()
                    .unwrap(),
                uri: svc.get("uri").expect("And a URI!").to_owned().unwrap(),
                extract: svc
                    .get("extract")
                    .expect("And extracted JSON!")
                    .to_owned()
                    .unwrap(),
            },
            None => panic!("No services specified!"),
        },
    };

    Ok(out)
}
