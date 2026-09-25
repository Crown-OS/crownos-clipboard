//! The crownos-clipboard daemon: wiring only.

use anyhow::Context;
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use crownos_clipboard::{Config, Daemon, ipc, wayland};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut event_loop: EventLoop<'static, Daemon> =
        EventLoop::try_new().context("creating the event loop")?;
    let handle = event_loop.handle();

    let (connection, queue, globals) =
        wayland::connect().context("connecting to the compositor")?;
    let mut daemon = Daemon::new(Config::load(), handle.clone(), queue.handle(), globals);
    WaylandSource::new(connection, queue)
        .insert(handle.clone())
        .map_err(|error| error.error)
        .context("registering the wayland connection with the event loop")?;

    let service = ipc::serve(&handle).context("binding the crownos-clipboard ipc socket")?;
    log::info!("managing the clipboard");

    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut daemon, |daemon| {
            ipc::publish(&service, daemon);
            if daemon.finished() {
                signal.stop();
            }
        })
        .context("running the event loop")
}
