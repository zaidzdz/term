mod pty;
mod renderer;
mod window;

use nix::libc::winsize;
use pty::PtyContext;
use window::TermWindow;
use winit::event_loop::EventLoop;

fn main() {
    let pty = PtyContext::new(winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    });
    let event_loop = EventLoop::new().expect("Could not create event loop");
    let term_window = pollster::block_on(TermWindow::new(&event_loop, pty));

    term_window.run(event_loop);
}
