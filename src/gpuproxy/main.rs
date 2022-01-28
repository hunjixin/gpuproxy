use gpuproxy::config::*;
use gpuproxy::proof_rpc::*;
use gpuproxy::models::*;
use gpuproxy::models::migrations::*;

use log::*;
use simplelog::*;
use clap::{App, AppSettings, Arg};
use std::sync::Arc;
use jsonrpc_http_server::ServerBuilder;
use jsonrpc_http_server::Server;
use crate::worker::Worker;
use crate::task_pool::*;
use anyhow::{Result};
use std::sync::{Mutex};
use std::env;

fn main() {
    TermLogger::init(LevelFilter::Trace, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    let app_m = App::new("gpuproxy")
        .version("0.0.1")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            App::new("run")
                .setting(AppSettings::ArgRequiredElseHelp)
                .about("run daemon for provide service")
                .args(&[
                    Arg::new("url")
                        .long("url")
                        .env("C2PROXY_URL")
                        .default_value("127.0.0.1:8888")
                        .help("specify url for provide service api service"),
                    Arg::new("db-dsn")
                        .long("db-dsn")
                        .env("C2PROXY_DSN")
                        .default_value("gpuproxy.db")
                        .help("specify sqlite path to store task"),
                    Arg::new("max-c2")
                        .long("max-c2")
                        .env("C2PROXY_MAX_C2")
                        .default_value("1")
                        .help("number of c2 task to run parallelly"),
                    Arg::new("disable-worker")
                        .long("disable-worker")
                        .env("C2PROXY_DISABLE_WORKER")
                        .required(false)
                        .takes_value(false)
                        .default_value("false")
                        .help("disable worker on gpuproxy manager"),
                ]),
        )
        .get_matches();

    match app_m.subcommand() {
        Some(("run", ref sub_m)) => {
            env::set_var("BELLMAN_NO_GPU", "1");
            let url: String = sub_m.value_of_t("url").unwrap_or_else(|e| e.exit());
            let max_c2: usize = sub_m.value_of_t("max-c2").unwrap_or_else(|e| e.exit());
            let db_dsn: String = sub_m.value_of_t("db-dsn").unwrap_or_else(|e| e.exit());
            let disable_worker: bool = sub_m.value_of_t("disable-worker").unwrap_or_else(|e| e.exit());
            let cfg = ServiceConfig::new(url, db_dsn, max_c2, disable_worker, "db".to_string(), "".to_string());
            run_cfg(cfg).unwrap().wait();
        } // run was used
        _ => {} // Either no subcommand or one not tested for...
    }
}

fn run_cfg(cfg: ServiceConfig) -> Result<Server> {
    let db_conn = establish_connection(cfg.db_dsn.as_str());
    run_db_migrations(&db_conn).expect("migrations error");
    let task_pool = task_pool::TaskpoolImpl::new(Mutex::new(db_conn));
    let worker_id = task_pool.get_worker_id()?;
    let arc_pool = Arc::new(task_pool);

    let resource: Arc<dyn resource::Resource + Send + Sync> =  if cfg.resource_type == "db" {
        arc_pool.clone()
    } else{
        Arc::new(resource::FileResource::new( cfg.resource_path.clone()))
    };


    let worker = worker::LocalWorker::new(cfg.max_c2, worker_id.to_string(), resource.clone(), arc_pool.clone());

   let io = proof::register(resource, arc_pool);
    if !cfg.disable_worker {
        worker.process_tasks();
        info!("ready for local worker address worker_id {}", worker_id);
    }

    let server = ServerBuilder::new(io)
        .start_http(&cfg.url.parse()?)
        .unwrap();

    info!("starting listening {}", cfg.url);
    Ok(server)
}//run cfg