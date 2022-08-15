pub fn init(name: String, use_stdout: bool) {
    if use_stdout {
        init_env_logger();
    } else {
        init_syslog(name);
    }
}

pub fn init_env_logger() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
}

pub fn init_syslog(name: String) {
    let formatter = syslog::Formatter3164 {
        facility: syslog::Facility::LOG_USER,
        hostname: None,
        process: name,
        pid: std::process::id(),
    };
    let logger = syslog::unix(formatter).expect("Could not connect to syslog");
    log::set_boxed_logger(Box::new(syslog::BasicLogger::new(logger)))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .expect("Could not set logger");
}
