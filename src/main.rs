use std::process::exit;
use tokio::signal;
use prometheus_exporter::prometheus::{register_int_gauge, IntGauge};
use rcon_client::{AuthRequest, RCONClient, RCONConfig, RCONRequest};
use tokio::time::sleep;
use lazy_static::lazy_static;
use tokio::signal::unix::{signal, SignalKind};

use argh::FromArgs;

lazy_static! {
    static ref PLAYERS_ONLINE_GAUGE: IntGauge =
        register_int_gauge!("factorio_players_online", "Number of players on the server currently").unwrap();
}

/// Factorio Prometheus Exporter
#[derive(FromArgs, Debug)]
struct Args {
    /// port to listen on
    #[argh(option, default = "9184")]
    port: u16,

    #[argh(option)]
    /// rcon host to connect to
    rcon_host: String,

    /// rcon port
    #[argh(option, default = "27015")]
    rcon_port: u16,

    /// rcon password
    #[argh(option)]
    rcon_password: Option<String>,

    /// file containing RCON password
    #[argh(option)]
    rcon_password_file: Option<String>,
}


fn extract_online_players(s: &str) -> Option<i64> {
    let prefix = "Online players (";
    let suffix = ")";

    if let Some(start) = s.find(prefix) {
        let rest = &s[start + prefix.len()..];
        if let Some(end) = rest.find(suffix) {
            let number_str = &rest[..end];
            return number_str.parse::<i64>().ok();
        }
    }
    None
}

#[tokio::main]
async fn main() {
    let args: Args = argh::from_env();

    if args.rcon_password.is_some() && args.rcon_password_file.is_some() {
        eprintln!("Cannot provide both --rcon-password and --rcon-password-file");
        exit(1);
    }

    let binding = format!("0.0.0.0:{}", args.port).parse().unwrap();
    prometheus_exporter::start(binding).unwrap();

    let mut client = match RCONClient::new(RCONConfig {
        url: format!("{}:{}", args.rcon_host, args.rcon_port),
        // Optional
        read_timeout: Some(13),
        write_timeout: Some(37),
    }) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error creating RCON client: {}", e);
            exit(1);
        }
    };
    
    let rcon_password = if let Some(password) = args.rcon_password {
        password
    } else if let Some(password_file) = args.rcon_password_file {
        std::fs::read_to_string(password_file).expect("Failed to read RCON password file")
    } else {
        eprintln!("No RCON password provided");
        exit(1);
    };

    println!("Connected to RCON server at {}:{}", args.rcon_host, args.rcon_port);

    let auth_result = client.auth(AuthRequest::new(rcon_password)).unwrap();

    if !auth_result.is_success() {
        eprintln!("Authentication failed: {}", auth_result.response_type);
        exit(2);
    }

    tokio::spawn(async move {
        let mut update_metrics = || {
            let players_online = match client.execute(RCONRequest::new("/players o".to_string())) {
                Ok(response) => {
                    match extract_online_players(&response.body) {
                        None => {
                            eprintln!("Failed to parse online players from: {}", response.body);
                            None
                        }
                        Some(amount) => {
                            Some(amount)
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error executing RCON request: {}", e);
                    None
                }
            };

            if let Some(players_online) = players_online {
                PLAYERS_ONLINE_GAUGE.set(players_online);
            }
        };

        loop {
            update_metrics();
            sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    let mut stream = signal(SignalKind::terminate())
        .expect("failed to create signal stream");


    tokio::select! {
        _ = stream.recv() => {
            println!("Received SIGTERM");
        }
        _ = signal::ctrl_c() => {
            println!("Received Ctrl-C");
        }
    }

    println!("shutting down gracefully");
}
