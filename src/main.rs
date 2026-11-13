fn main() {
    // EC5-24: when the reader goes away (`colctl search | head`), stop quietly instead of panicking
    // about a broken pipe. Exit 141, as a process killed by SIGPIPE would.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !broken_pipe(info.payload()) {
            default(info);
        }
    }));
    let code = std::panic::catch_unwind(|| colctl::cli::run(std::env::args_os())).unwrap_or_else(
        |payload| {
            if broken_pipe(payload.as_ref()) {
                141
            } else {
                101
            }
        },
    );
    std::process::exit(code);
}

/// True when a panic is `println!` failing because stdout's reader closed it.
fn broken_pipe(payload: &(dyn std::any::Any + Send)) -> bool {
    let text = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    text.contains("Broken pipe")
}
